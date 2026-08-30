use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_exit, expect_stderr_nonempty, expect_success, parse_json};

#[e2e(
    story = "PARK-INSPECT-006",
    scope = "lookup-errors",
    priority = "P0",
    description = "Report missing records consistently across operations",
    tags = ["inspection", "errors", "exit-codes"]
)]
pub fn report_missing_records_consistently() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-INSPECT-006")?;

    expect_human_missing(&environment, "status", &["status", "missing"])?;
    expect_json_missing(
        &environment,
        "status JSON",
        &["status", "missing", "--json"],
    )?;
    expect_human_missing(&environment, "logs", &["logs", "missing"])?;
    expect_json_missing(
        &environment,
        "logs JSON",
        &["logs", "missing", "--json"],
    )?;
    expect_human_missing(&environment, "stop", &["stop", "missing"])?;
    expect_human_missing(&environment, "restart", &["restart", "missing"])?;
    expect_human_missing(&environment, "start", &["start", "missing"])?;
    expect_human_missing(
        &environment,
        "signal",
        &["signal", "missing", "TERM"],
    )?;
    expect_human_missing(&environment, "remove", &["rm", "missing"])?;
    expect_human_missing(
        &environment,
        "wait",
        &["wait", "missing", "--exit"],
    )?;

    let ps = environment.run(&["ps", "--json"])?;
    expect_success("ps after missing lookups", &ps)?;
    let json = parse_json("ps after missing lookups", &ps)?;
    if json.get("data").and_then(serde_json::Value::as_array).is_none_or(|records| !records.is_empty()) {
        return Err(format!("missing lookups created a record: {json}"));
    }
    Ok(())
}

fn expect_human_missing(
    environment: &TestEnvironment,
    operation: &str,
    arguments: &[&str],
) -> Result<(), String> {
    let output = environment.run(arguments)?;
    expect_exit(operation, &output, 3)?;
    if !output.stdout.is_empty() {
        return Err(format!(
            "{operation} wrote unexpected stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        ));
    }
    expect_stderr_nonempty(operation, &output)
}

fn expect_json_missing(
    environment: &TestEnvironment,
    operation: &str,
    arguments: &[&str],
) -> Result<(), String> {
    let output = environment.run(arguments)?;
    expect_exit(operation, &output, 3)?;
    if !output.stderr.is_empty() {
        return Err(format!(
            "{operation} wrote unexpected stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let json = parse_json(operation, &output)?;
    if json.get("status").and_then(serde_json::Value::as_str) != Some("missing_record")
        || json.get("ok").and_then(serde_json::Value::as_bool) != Some(false)
        || json
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(serde_json::Value::as_str)
            != Some("missing_record")
    {
        return Err(format!("{operation} returned an unexpected result: {json}"));
    }
    Ok(())
}
