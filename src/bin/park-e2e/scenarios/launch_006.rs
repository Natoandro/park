use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-LAUNCH-006",
    scope = "terminal-records",
    priority = "P0",
    description = "Persist a naturally exited command's exit code",
    tags = ["launch", "lifecycle", "inspection"]
)]
pub fn record_successful_exit_code() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LAUNCH-006")?;

    let launch = environment.run(&[
        "exit-code",
        "--",
        "/bin/sh",
        "-c",
        "exit 7",
    ])?;
    expect_success("launch", &launch)?;
    let wait = environment.run(&["wait", "exit-code", "--exit"])?;
    expect_success("wait", &wait)?;

    let status = environment.run(&["status", "exit-code", "--json"])?;
    expect_success("status", &status)?;
    let json = parse_json("status", &status)?;
    let record = json
        .get("data")
        .ok_or_else(|| "status response is missing data".to_owned())?;
    if record.get("state").and_then(|value| value.as_str()) != Some("exited")
        || record.get("exit_code").and_then(|value| value.as_i64()) != Some(7)
        || record.get("exited_at").and_then(|value| value.as_u64()).is_none()
    {
        return Err(format!("exit outcome was not persisted correctly: {record}"));
    }
    let logs = environment.run(&["logs", "exit-code"])?;
    expect_success("logs after exit", &logs)?;
    Ok(())
}
