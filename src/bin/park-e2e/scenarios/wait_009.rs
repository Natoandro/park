use std::env;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_contains, expect_success, parse_json};

#[e2e(
    story = "PARK-WAIT-009",
    scope = "wait-disconnect",
    priority = "P1",
    description = "Release a daemon wait when its client disconnects",
    tags = ["wait", "ipc", "concurrency"]
)]
pub fn disconnect_wait_client_safely() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-WAIT-009")?;
    let launch = environment.run(&["disconnected", "--", "/bin/sleep", "30"])?;
    expect_success("launch", &launch)?;

    let mut waiter = spawn_wait_client(&environment)?;
    for _ in 0..20 {
        if let Some(status) = waiter
            .try_wait()
            .map_err(|error| format!("check wait client: {error}"))?
        {
            return Err(format!("wait client exited before disconnect: {status}"));
        }
        thread::sleep(Duration::from_millis(10));
    }
    waiter
        .kill()
        .map_err(|error| format!("disconnect wait client: {error}"))?;
    waiter
        .wait()
        .map_err(|error| format!("reap disconnected wait client: {error}"))?;

    let started = Instant::now();
    let status = environment.run(&["status", "disconnected", "--json"])?;
    if started.elapsed() > Duration::from_secs(2) {
        return Err("status was blocked by the disconnected wait".to_owned());
    }
    expect_success("status after disconnect", &status)?;
    let status_json = parse_json("status after disconnect", &status)?;
    let record = status_json
        .get("data")
        .ok_or_else(|| "status after disconnect has no record".to_owned())?;
    if record.get("state").and_then(|value| value.as_str()) != Some("running") {
        return Err(format!("managed process changed state after disconnect: {record}"));
    }

    let started = Instant::now();
    let stop = environment.run(&["stop", "disconnected", "--force"])?;
    if started.elapsed() > Duration::from_secs(3) {
        return Err("stop was blocked by the disconnected wait".to_owned());
    }
    expect_success("stop after disconnect", &stop)?;
    expect_contains(
        &String::from_utf8_lossy(&stop.stdout),
        "killed",
    )?;
    Ok(())
}

fn spawn_wait_client(environment: &TestEnvironment) -> Result<Child, String> {
    let park = env::var_os("PARK_BIN").ok_or_else(|| "PARK_BIN is not set".to_owned())?;
    Command::new(park)
        .args(["wait", "disconnected", "--exit"])
        .current_dir(environment.project_path())
        .env("HOME", environment.root_path().join("home"))
        .env("XDG_STATE_HOME", environment.root_path().join("state"))
        .env("XDG_RUNTIME_DIR", environment.root_path().join("runtime"))
        .env("PARK_E2E_SCENARIO", "PARK-WAIT-009")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("spawn wait client: {error}"))
}
