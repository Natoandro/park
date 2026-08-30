use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-INSPECT-005",
    scope = "machine-readable-inspection",
    priority = "P0",
    description = "Return stable JSON status before and after termination",
    tags = ["inspection", "status", "json"]
)]
pub fn return_stable_json_for_status() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-INSPECT-005")?;
    let launch = environment.run(&["json-status", "--", "/bin/sleep", "30"])?;
    expect_success("launch", &launch)?;

    let before = environment.run(&["status", "json-status", "--json"])?;
    expect_success("status before termination", &before)?;
    let before_json = assert_status_json("status before termination", &before, "running")?;
    let before_record = before_json
        .get("data")
        .ok_or_else(|| "running status has no record".to_owned())?;
    if before_record.get("started_at").and_then(serde_json::Value::as_u64).is_none()
        || !before_record.get("exited_at").is_some_and(serde_json::Value::is_null)
    {
        return Err(format!("running status has inconsistent timestamps: {before_record}"));
    }

    let stop = environment.run(&["stop", "json-status", "--force"])?;
    expect_success("force stop", &stop)?;
    let after = environment.run(&["status", "json-status", "--json"])?;
    expect_success("status after termination", &after)?;
    let after_json = assert_status_json("status after termination", &after, "killed")?;
    let after_record = after_json
        .get("data")
        .ok_or_else(|| "terminal status has no record".to_owned())?;
    if after_record
        .get("exited_at")
        .and_then(serde_json::Value::as_u64)
        .is_none()
        || after_record
            .get("termination_signal")
            .and_then(serde_json::Value::as_i64)
            != Some(9)
    {
        return Err(format!("terminal status has inconsistent outcome: {after_record}"));
    }
    Ok(())
}

fn assert_status_json(
    operation: &str,
    output: &std::process::Output,
    expected_state: &str,
) -> Result<serde_json::Value, String> {
    if !output.stderr.is_empty() {
        return Err(format!(
            "{operation} wrote unexpected stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let json = parse_json(operation, output)?;
    if json.get("status").and_then(serde_json::Value::as_str) != Some("success")
        || json.get("ok").and_then(serde_json::Value::as_bool) != Some(true)
    {
        return Err(format!("{operation} returned an unexpected result: {json}"));
    }
    let record = json
        .get("data")
        .ok_or_else(|| format!("{operation} has no record data"))?;
    if record.get("state").and_then(serde_json::Value::as_str) != Some(expected_state) {
        return Err(format!(
            "{operation} reported the wrong state: expected {expected_state}, got {record}"
        ));
    }
    for field in [
        "key",
        "working_directory",
        "executable",
        "arguments",
        "created_at",
        "started_at",
        "exited_at",
        "state",
        "exit_code",
        "termination_signal",
        "logs",
    ] {
        if record.get(field).is_none() {
            return Err(format!("{operation} is missing stable field {field:?}: {record}"));
        }
    }
    Ok(json)
}
