use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-DAEMON-001",
    scope = "daemon-startup",
    priority = "P0",
    description = "Start the daemon automatically on first use",
    tags = ["daemon", "startup", "smoke"]
)]
pub fn start_daemon_on_first_use() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-DAEMON-001")?;

    let first = environment.run(&["ps", "--json"])?;
    expect_success("first ps", &first)?;
    let first_json = parse_json("first ps", &first)?;
    if first_json.get("status").and_then(|value| value.as_str()) != Some("success")
        || first_json.get("data").and_then(|value| value.as_array()).is_none()
    {
        return Err(format!("first request returned an unexpected result: {first_json}"));
    }

    let second = environment.run(&["ps", "--json"])?;
    expect_success("second ps", &second)?;
    let second_json = parse_json("second ps", &second)?;
    if first_json != second_json {
        return Err(format!(
            "subsequent request did not observe the same registry: first={first_json}, second={second_json}"
        ));
    }
    Ok(())
}
