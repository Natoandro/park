use park_e2e_macros::e2e;
use serde_json::Value;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_exit, expect_success, parse_json};

#[e2e(
    story = "PARK-SAFETY-005",
    scope = "json-output",
    priority = "P0",
    description = "Keep JSON inspection results parseable and separate from diagnostics",
    tags = ["safety", "cli", "json", "errors"]
)]
pub fn keep_json_results_free_of_diagnostics() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-SAFETY-005")?;

    let ps = environment.run(&["ps", "--json"])?;
    expect_success("empty ps", &ps)?;
    let ps_json = expect_json("empty ps", &ps)?;
    expect_success_envelope("empty ps", &ps_json)?;
    if !ps_json
        .get("data")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
    {
        return Err(format!("empty ps returned unexpected data: {ps_json}"));
    }

    let missing_status = environment.run(&["status", "missing", "--json"])?;
    expect_json_failure("missing status", &missing_status, "missing_record", 3)?;

    let missing_logs = environment.run(&["logs", "missing", "--json"])?;
    expect_json_failure("missing logs", &missing_logs, "missing_record", 3)?;

    let launch = environment.run(&[
        "json-output",
        "--",
        "/bin/sh",
        "-c",
        "printf json-stdout; printf json-stderr >&2",
    ])?;
    expect_success("launch", &launch)?;
    let wait = environment.run(&["wait", "json-output", "--exit"])?;
    expect_success("wait", &wait)?;

    let status = environment.run(&["status", "json-output", "--json"])?;
    let status_json = expect_json("status", &status)?;
    expect_success_envelope("status", &status_json)?;
    if status_json
        .get("data")
        .and_then(|data| data.get("state"))
        .and_then(Value::as_str)
        != Some("exited")
    {
        return Err(format!("status JSON has unexpected data: {status_json}"));
    }

    let logs = environment.run(&["logs", "json-output", "--json"])?;
    let logs_json = expect_json("logs", &logs)?;
    expect_success_envelope("logs", &logs_json)?;
    let data = logs_json
        .get("data")
        .ok_or_else(|| "logs JSON is missing data".to_owned())?;
    if data.get("stream").and_then(Value::as_str) != Some("combined")
        || data.get("content").and_then(Value::as_str) != Some("json-stdoutjson-stderr")
    {
        return Err(format!("logs JSON has unexpected data: {logs_json}"));
    }
    Ok(())
}

fn expect_json(operation: &str, output: &std::process::Output) -> Result<Value, String> {
    if !output.stderr.is_empty() {
        return Err(format!(
            "{operation} wrote a human diagnostic to stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    parse_json(operation, output)
}

fn expect_success_envelope(operation: &str, value: &Value) -> Result<(), String> {
    if value.get("status").and_then(Value::as_str) != Some("success")
        || value.get("ok").and_then(Value::as_bool) != Some(true)
        || value.get("error").is_some()
    {
        return Err(format!("{operation} returned an invalid success envelope: {value}"));
    }
    Ok(())
}

fn expect_json_failure(
    operation: &str,
    output: &std::process::Output,
    expected_status: &str,
    expected_exit: i32,
) -> Result<(), String> {
    expect_exit(operation, output, expected_exit)?;
    let value = expect_json(operation, output)?;
    if value.get("status").and_then(Value::as_str) != Some(expected_status)
        || value.get("ok").and_then(Value::as_bool) != Some(false)
        || value
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str)
            != Some(expected_status)
    {
        return Err(format!("{operation} returned an invalid error envelope: {value}"));
    }
    Ok(())
}
