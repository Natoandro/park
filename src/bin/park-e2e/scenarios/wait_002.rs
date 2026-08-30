use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_exit, expect_success, parse_json};

#[e2e(
    story = "PARK-WAIT-002",
    scope = "wait-terminal",
    priority = "P0",
    description = "Wait for every terminal process outcome",
    tags = ["wait", "lifecycle", "terminal"]
)]
pub fn wait_for_any_terminal_exit() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-WAIT-002")?;

    let exited_launch = environment.run(&["exited", "--", "/bin/true"])?;
    expect_success("exited launch", &exited_launch)?;
    let exited_wait = environment.run(&["wait", "exited", "--exit"])?;
    expect_success("exited wait", &exited_wait)?;
    let exited_record = parse_json("exited wait", &exited_wait)?;
    if exited_record.get("state").and_then(|value| value.as_str()) != Some("exited") {
        return Err(format!("terminal wait returned a non-exited record: {exited_record}"));
    }

    let failed_launch = environment.run(&["failed", "--", "/definitely/missing/park-command"])?;
    expect_exit("failed launch", &failed_launch, 1)?;
    let failed_wait = environment.run(&["wait", "failed", "--exit"])?;
    expect_success("failed wait", &failed_wait)?;
    let failed_record = parse_json("failed wait", &failed_wait)?;
    if failed_record.get("state").and_then(|value| value.as_str()) != Some("failed")
        || failed_record
            .get("failure_reason")
            .and_then(|value| value.as_str())
            .is_none()
    {
        return Err(format!("terminal wait returned a non-failed record: {failed_record}"));
    }

    let killed_launch = environment.run(&["killed", "--", "/bin/sleep", "30"])?;
    expect_success("killed launch", &killed_launch)?;
    let premature = environment.run(&["wait", "killed", "--exit", "--timeout", "0ms"])?;
    expect_exit("premature terminal wait", &premature, 1)?;
    let stop = environment.run(&["stop", "killed", "--force"])?;
    expect_success("force stop", &stop)?;
    let killed_wait = environment.run(&["wait", "killed", "--exit"])?;
    expect_success("killed wait", &killed_wait)?;
    let killed_record = parse_json("killed wait", &killed_wait)?;
    if killed_record.get("state").and_then(|value| value.as_str()) != Some("killed")
        || killed_record
            .get("termination_signal")
            .and_then(|value| value.as_i64())
            .is_none()
    {
        return Err(format!("terminal wait returned a non-killed record: {killed_record}"));
    }
    Ok(())
}
