use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_exit, expect_success, parse_json};

#[e2e(
    story = "PARK-LAUNCH-007",
    scope = "spawn-failure",
    priority = "P0",
    description = "Persist and inspect a failed command spawn",
    tags = ["launch", "errors", "lifecycle"]
)]
pub fn record_spawn_failure() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LAUNCH-007")?;
    let command = "/definitely/missing/park-e2e-command";

    let launch = environment.run(&["failed-spawn", "--", command])?;
    expect_exit("failed launch", &launch, 1)?;

    let status = environment.run(&["status", "failed-spawn", "--json"])?;
    expect_success("status", &status)?;
    let json = parse_json("status", &status)?;
    let record = json
        .get("data")
        .ok_or_else(|| "failed status response is missing data".to_owned())?;
    if record.get("state").and_then(|value| value.as_str()) != Some("failed") {
        return Err(format!("spawn failure has the wrong state: {record}"));
    }
    if record
        .get("failure_reason")
        .and_then(|value| value.as_str())
        .is_none_or(|reason| !reason.contains("spawn"))
    {
        return Err(format!("spawn failure has no diagnostic: {record}"));
    }

    let logs = environment.run(&["logs", "failed-spawn", "--json"])?;
    expect_success("failure logs", &logs)?;
    let logs_json = parse_json("failure logs", &logs)?;
    if logs_json
        .get("data")
        .and_then(|data| data.get("state"))
        .and_then(|state| state.as_str())
        != Some("failed")
    {
        return Err(format!("failure logs have the wrong state: {logs_json}"));
    }

    let restart = environment.run(&["restart", "failed-spawn"])?;
    expect_exit("restart failed command", &restart, 1)?;
    let remove = environment.run(&["rm", "failed-spawn"])?;
    expect_success("remove failed record", &remove)?;
    Ok(())
}
