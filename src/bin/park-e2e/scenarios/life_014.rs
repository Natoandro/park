use std::env;
use std::process::{Command, Output};
use std::thread;

use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-LIFE-014",
    scope = "lifecycle-concurrency",
    priority = "P0",
    description = "Serialize concurrent lifecycle mutations for one record",
    tags = ["lifecycle", "concurrency", "linux", "system", "stress"]
)]
pub fn serialize_concurrent_lifecycle_mutations() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LIFE-014")?;
    let launch = environment.run(&[
        "race-target",
        "--",
        "/bin/sh",
        "-c",
        "trap '' USR1; sleep 30",
    ])?;
    expect_success("launch", &launch)?;
    expect_success(
        "wait for running",
        &environment.run(&["wait", "race-target", "--state", "running"])? ,
    )?;

    let operations: [&[&str]; 4] = [
        &["stop", "race-target"],
        &["restart", "race-target"],
        &["signal", "race-target", "USR1"],
        &["rm", "race-target"],
    ];
    let clients = operations
        .into_iter()
        .map(|arguments| spawn_client(&environment, arguments))
        .collect::<Vec<_>>();
    for client in clients {
        let output = client
            .join()
            .map_err(|_| "concurrent lifecycle client panicked".to_owned())??;
        let code = output.status.code();
        if !matches!(code, Some(0 | 3 | 5)) {
            return Err(format!(
                "concurrent lifecycle request had an unstable result {code:?}; stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    let status = environment.run(&["status", "race-target", "--json"])?;
    let status_code = status.status.code();
    if status_code == Some(3) {
        let logs = environment.run(&["logs", "race-target", "--json"])?;
        if logs.status.code() != Some(3) {
            return Err("a removed record left addressable lifecycle logs".to_owned());
        }
        return Ok(());
    }
    expect_success("final status", &status)?;
    let value = parse_json("final status", &status)?;
    let state = value
        .get("data")
        .and_then(|data| data.get("state"))
        .and_then(|state| state.as_str());
    if !matches!(state, Some("running" | "exited" | "failed" | "killed")) {
        return Err(format!("concurrent mutations left an invalid state: {value}"));
    }
    if state == Some("running")
        && (value
            .get("data")
            .and_then(|data| data.get("pid"))
            .and_then(|pid| pid.as_u64())
            .is_none()
            || value
                .get("data")
                .and_then(|data| data.get("process_group_id"))
                .and_then(|group| group.as_u64())
                .is_none())
    {
        return Err(format!("running record has incomplete process identity: {value}"));
    }
    let logs = environment.run(&["logs", "race-target", "--json"])?;
    expect_success("final logs", &logs)?;
    Ok(())
}

fn spawn_client(environment: &TestEnvironment, arguments: &[&str]) -> thread::JoinHandle<Result<Output, String>> {
    let park = match env::var_os("PARK_BIN") {
        Some(park) => park,
        None => return thread::spawn(|| Err("PARK_BIN is not set".to_owned())),
    };
    let root = environment.root_path().to_path_buf();
    let project = environment.project_path().to_path_buf();
    let arguments = arguments.iter().map(|argument| (*argument).to_owned()).collect::<Vec<_>>();
    thread::spawn(move || {
        Command::new(park)
            .args(&arguments)
            .current_dir(project)
            .env("HOME", root.join("home"))
            .env("XDG_STATE_HOME", root.join("state"))
            .env("XDG_RUNTIME_DIR", root.join("runtime"))
            .env("PARK_E2E_SCENARIO", "PARK-LIFE-014")
            .output()
            .map_err(|error| format!("execute concurrent Park client: {error}"))
    })
}
