use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-INSPECT-002",
    scope = "process-listing",
    priority = "P1",
    description = "Sort ps records deterministically by name",
    tags = ["inspection", "ps", "ordering"]
)]
pub fn sort_ps_deterministically() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-INSPECT-002")?;
    for name in ["zeta", "alpha", "middle"] {
        let launch = environment.run(&[name, "--", "/bin/true"])?;
        expect_success("launch", &launch)?;
        let wait = environment.run(&["wait", name, "--exit"])?;
        expect_success("wait", &wait)?;
    }

    let first = environment.run(&["ps", "--json"])?;
    expect_success("first ps", &first)?;
    let first_json = assert_ps_response("first ps", &first)?;
    let first_names = ps_names(&first_json)?;
    let expected = ["alpha".to_owned(), "middle".to_owned(), "zeta".to_owned()];
    if first_names != expected {
        return Err(format!(
            "ps names were not sorted: expected {expected:?}, got {first_names:?}"
        ));
    }

    let second = environment.run(&["ps", "--json"])?;
    expect_success("second ps", &second)?;
    let second_json = parse_json("second ps", &second)?;
    if first_json != second_json {
        return Err(format!(
            "repeated ps calls changed their ordering or fields: first={first_json}, second={second_json}"
        ));
    }
    Ok(())
}

fn assert_ps_response(operation: &str, output: &std::process::Output) -> Result<serde_json::Value, String> {
    if !output.stderr.is_empty() {
        return Err(format!(
            "{operation} wrote unexpected stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let json = parse_json(operation, output)?;
    if json.get("status").and_then(serde_json::Value::as_str) != Some("success")
        || json.get("ok").and_then(serde_json::Value::as_bool) != Some(true)
        || json.get("data").and_then(serde_json::Value::as_array).is_none()
    {
        return Err(format!("{operation} returned an unexpected response: {json}"));
    }
    Ok(json)
}

fn ps_names(json: &serde_json::Value) -> Result<Vec<String>, String> {
    json.get("data")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "ps response has no data array".to_owned())?
        .iter()
        .map(|record| {
            record
                .get("key")
                .and_then(|key| key.get("name"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| format!("ps record has no encoded name: {record}"))
        })
        .collect()
}
