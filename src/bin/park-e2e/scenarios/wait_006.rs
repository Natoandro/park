use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-WAIT-006",
    scope = "wait-validation",
    priority = "P2",
    description = "Treat an empty output match as immediately satisfied",
    tags = ["wait", "validation"]
)]
pub fn match_empty_text_immediately() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-WAIT-006")?;
    let launch = environment.run(&["empty-match", "--", "/bin/sleep", "30"])?;
    expect_success("launch", &launch)?;

    let matched = environment.run(&["wait", "empty-match", "--match", ""])?;
    expect_success("empty match", &matched)?;
    let record = parse_json("empty match", &matched)?;
    if record.get("state").and_then(|value| value.as_str()) != Some("running")
        || record.get("pid").and_then(|value| value.as_u64()).is_none()
    {
        return Err(format!("empty match returned the wrong record: {record}"));
    }

    let stop = environment.run(&["stop", "empty-match", "--force"])?;
    expect_success("force stop", &stop)?;
    Ok(())
}
