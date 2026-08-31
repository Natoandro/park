use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_contains, expect_success, parse_json};

#[e2e(
    story = "PARK-LAUNCH-002",
    scope = "launch-transaction",
    priority = "P0",
    description = "Make a successful launch immediately inspectable",
    tags = ["launch", "inspection", "smoke"]
)]
pub fn record_before_reporting_success() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LAUNCH-002")?;

    let launch = environment.run(&["inspectable", "--", "/bin/sleep", "30"])?;
    expect_success("launch", &launch)?;

    let status = environment.run(&["status", "inspectable", "--json"])?;
    expect_success("immediate status", &status)?;
    let status_json = parse_json("immediate status", &status)?;
    let record = status_json
        .get("data")
        .ok_or_else(|| "immediate status is missing its record".to_owned())?;
    if record.get("pid").and_then(|value| value.as_u64()).is_none() {
        return Err(format!("immediate status has no process identity: {record}"));
    }
    expect_contains(
        &record.to_string(),
        &format!(r#""state":"running""#),
    )?;
    let project = environment
        .project_path()
        .to_str()
        .ok_or_else(|| "project path is not valid UTF-8".to_owned())?;
    expect_contains(
        &record.to_string(),
        &format!(r#""working_directory":"{project}""#),
    )?;

    let records = environment.run(&["ps", "--json"])?;
    expect_success("immediate ps", &records)?;
    expect_contains(
        &String::from_utf8_lossy(&records.stdout),
        "inspectable",
    )?;
    let stop = environment.run(&["stop", "inspectable", "--force"])?;
    expect_success("stop", &stop)?;
    Ok(())
}
