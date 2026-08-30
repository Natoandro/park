use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_exit, expect_success};

#[e2e(
    story = "PARK-SAFETY-004",
    scope = "cli-output",
    priority = "P0",
    description = "Keep representative CLI outcomes non-interactive with stable exit codes",
    tags = ["safety", "cli", "errors", "non-interactive"]
)]
pub fn keep_cli_outcomes_script_friendly() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-SAFETY-004")?;

    let missing = environment.run(&["status", "missing"])?;
    expect_human_failure("missing status", &missing, 3)?;

    let usage = environment.run(&["status"])?;
    expect_human_failure("usage error", &usage, 2)?;

    let launch = environment.run(&["timeout", "--", "/bin/sleep", "30"])?;
    expect_success("launch", &launch)?;
    expect_stdout_only("launch", &launch)?;

    let timeout = environment.run(&["wait", "timeout", "--exit", "--timeout", "0ms"])?;
    expect_human_failure("wait timeout", &timeout, 1)?;

    let stop = environment.run(&["stop", "timeout", "--force"])?;
    expect_success("force stop", &stop)?;
    expect_stdout_only("force stop", &stop)?;

    let terminal_launch = environment.run(&["terminal", "--", "/bin/true"])?;
    expect_success("terminal launch", &terminal_launch)?;
    let wait = environment.run(&["wait", "terminal", "--exit"])?;
    expect_success("terminal wait", &wait)?;

    let invalid_state = environment.run(&["stop", "terminal"])?;
    expect_human_failure("invalid-state stop", &invalid_state, 5)?;
    Ok(())
}

fn expect_human_failure(
    operation: &str,
    output: &std::process::Output,
    expected_exit: i32,
) -> Result<(), String> {
    expect_exit(operation, output, expected_exit)?;
    if output.stdout.is_empty() {
        return expect_stderr(operation, output);
    }
    Err(format!(
        "{operation} wrote unexpected stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    ))
}

fn expect_stderr(operation: &str, output: &std::process::Output) -> Result<(), String> {
    if !output.stderr.is_empty() {
        return Ok(());
    }
    Err(format!("{operation} did not write a diagnostic to stderr"))
}

fn expect_stdout_only(operation: &str, output: &std::process::Output) -> Result<(), String> {
    if output.stderr.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{operation} wrote unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    ))
}
