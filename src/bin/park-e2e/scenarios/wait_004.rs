use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-WAIT-004",
    scope = "wait-history",
    priority = "P1",
    description = "Match output retained from a completed process",
    tags = ["wait", "logs", "history"]
)]
pub fn match_historical_output() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-WAIT-004")?;
    let launch = environment.run(&[
        "historical",
        "--",
        "/bin/sh",
        "-c",
        "printf 'historical[.*]' >&2",
    ])?;
    expect_success("launch", &launch)?;
    expect_success(
        "wait for exit",
        &environment.run(&["wait", "historical", "--exit"] )?,
    )?;

    let matched = environment.run(&[
        "wait",
        "historical",
        "--match",
        "historical[.*]",
        "--timeout",
        "1s",
    ])?;
    expect_success("historical match", &matched)?;
    let record = parse_json("historical match", &matched)?;
    if record.get("state").and_then(|value| value.as_str()) != Some("exited") {
        return Err(format!("historical match returned the wrong record: {record}"));
    }
    Ok(())
}
