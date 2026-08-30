use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-INSPECT-004",
    scope = "machine-readable-inspection",
    priority = "P0",
    description = "Return stable JSON for ps listing",
    tags = ["inspection", "ps", "json"]
)]
pub fn return_stable_json_for_ps() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-INSPECT-004")?;
    let launch = environment.run(&["json-ps", "--", "/bin/true"])?;
    expect_success("launch", &launch)?;
    let wait = environment.run(&["wait", "json-ps", "--exit"])?;
    expect_success("wait", &wait)?;

    let first = environment.run(&["ps", "--json"])?;
    expect_success("first ps", &first)?;
    let first_json = assert_ps_json("first ps", &first)?;
    assert_record_shape(&first_json)?;

    let second = environment.run(&["ps", "--json"])?;
    expect_success("second ps", &second)?;
    let second_json = assert_ps_json("second ps", &second)?;
    if first_json != second_json {
        return Err(format!(
            "repeated ps JSON was not stable: first={first_json}, second={second_json}"
        ));
    }
    Ok(())
}

fn assert_ps_json(operation: &str, output: &std::process::Output) -> Result<serde_json::Value, String> {
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
    let records = json
        .get("data")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("{operation} has no data array"))?;
    if records.len() != 1 {
        return Err(format!("{operation} returned {records:?}, expected one record"));
    }
    Ok(json)
}

fn assert_record_shape(json: &serde_json::Value) -> Result<(), String> {
    let record = json
        .get("data")
        .and_then(serde_json::Value::as_array)
        .and_then(|records| records.first())
        .ok_or_else(|| "ps JSON has no record".to_owned())?;
    for field in [
        "key",
        "working_directory",
        "executable",
        "arguments",
        "pid",
        "process_group_id",
        "created_at",
        "started_at",
        "exited_at",
        "state",
        "exit_code",
        "termination_signal",
        "failure_reason",
        "logs",
    ] {
        if record.get(field).is_none() {
            return Err(format!("ps record is missing stable field {field:?}: {record}"));
        }
    }
    Ok(())
}
