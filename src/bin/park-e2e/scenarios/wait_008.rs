use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{
    TestEnvironment, expect_contains, expect_exit, expect_stderr_nonempty, parse_json,
};

#[e2e(
    story = "PARK-WAIT-008",
    scope = "wait-usage-errors",
    priority = "P0",
    description = "Reject malformed and ambiguous wait conditions",
    tags = ["wait", "cli", "errors"]
)]
pub fn reject_invalid_wait_conditions() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-WAIT-008")?;
    assert_usage_error(&environment, "missing condition", &["wait", "invalid"])?;
    assert_usage_error(
        &environment,
        "two conditions",
        &["wait", "invalid", "--exit", "--state", "running"],
    )?;
    let invalid_state = assert_usage_error(
        &environment,
        "invalid state",
        &["wait", "invalid", "--state", "unknown"],
    )?;
    expect_contains(&String::from_utf8_lossy(&invalid_state.stderr), "invalid state")?;
    let invalid_duration = assert_usage_error(
        &environment,
        "invalid duration",
        &["wait", "invalid", "--exit", "--timeout", "soon"],
    )?;
    expect_contains(
        &String::from_utf8_lossy(&invalid_duration.stderr),
        "invalid duration",
    )?;

    let records = environment.run(&["ps", "--json"])?;
    expect_exit("ps", &records, 0)?;
    let records = parse_json("ps", &records)?;
    if !records
        .get("data")
        .and_then(|data| data.as_array())
        .is_some_and(Vec::is_empty)
    {
        return Err(format!("invalid waits created records: {records}"));
    }
    Ok(())
}

fn assert_usage_error(
    environment: &TestEnvironment,
    operation: &str,
    arguments: &[&str],
) -> Result<std::process::Output, String> {
    let output = environment.run(arguments)?;
    expect_exit(operation, &output, 2)?;
    expect_stderr_nonempty(operation, &output)?;
    if !output.stdout.is_empty() {
        return Err(format!("{operation} wrote unexpected stdout"));
    }
    Ok(output)
}
