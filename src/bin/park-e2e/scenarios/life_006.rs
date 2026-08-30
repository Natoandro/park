use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_exit, expect_stderr_nonempty, expect_success, parse_json};

#[e2e(
    story = "PARK-LIFE-006",
    scope = "signal-validation",
    priority = "P1",
    description = "Reject numeric and unknown signal values without affecting a process",
    tags = ["lifecycle", "signals", "errors"]
)]
pub fn reject_unsupported_signals() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LIFE-006")?;
    let launch = environment.run(&["signal-validation", "--", "/bin/sleep", "30"])?;
    expect_success("launch", &launch)?;
    expect_success(
        "wait for running",
        &environment.run(&["wait", "signal-validation", "--state", "running"])? ,
    )?;

    for signal in ["9", "NOT_A_SIGNAL"] {
        let output = environment.run(&["signal", "signal-validation", signal])?;
        expect_exit(signal, &output, 1)?;
        expect_stderr_nonempty(signal, &output)?;
    }

    let status = environment.run(&["status", "signal-validation", "--json"])?;
    expect_success("status after rejected signals", &status)?;
    let value = parse_json("status after rejected signals", &status)?;
    if value
        .get("data")
        .and_then(|data| data.get("state"))
        .and_then(|state| state.as_str())
        != Some("running")
    {
        return Err(format!("rejected signal changed the process: {value}"));
    }
    expect_success(
        "cleanup",
        &environment.run(&["stop", "signal-validation", "--force"])? ,
    )?;
    Ok(())
}
