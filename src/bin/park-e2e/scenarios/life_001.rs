use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-LIFE-001",
    scope = "stop",
    priority = "P0",
    description = "Stop a process gracefully and retain its cleanup output",
    tags = ["lifecycle", "stop", "smoke"]
)]
pub fn stop_gracefully() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LIFE-001")?;
    let launch = environment.run(&[
        "graceful",
        "--",
        "/bin/sh",
        "-c",
        "trap 'printf graceful-cleanup' TERM; sleep 30",
    ])?;
    expect_success("launch", &launch)?;
    expect_success(
        "wait for running",
        &environment.run(&["wait", "graceful", "--state", "running"])? ,
    )?;

    let stop = environment.run(&["stop", "graceful"])?;
    expect_success("graceful stop", &stop)?;

    let status = environment.run(&["status", "graceful", "--json"])?;
    expect_success("status", &status)?;
    let value = parse_json("status", &status)?;
    let state = value
        .get("data")
        .and_then(|data| data.get("state"))
        .and_then(|state| state.as_str());
    if !matches!(state, Some("exited" | "failed" | "killed")) {
        return Err(format!("graceful stop did not produce a terminal state: {value}"));
    }

    let logs = environment.run(&["logs", "graceful", "--stdout"])?;
    expect_success("cleanup logs", &logs)?;
    if logs.stdout != b"graceful-cleanup" {
        return Err(format!(
            "cleanup output differs: {:?}",
            String::from_utf8_lossy(&logs.stdout)
        ));
    }
    Ok(())
}
