use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_contains, expect_success};

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
    expect_contains(&String::from_utf8_lossy(&matched.stdout), "State: running")?;
    expect_contains(&String::from_utf8_lossy(&matched.stdout), "PID: ")?;

    let stop = environment.run(&["stop", "empty-match", "--force"])?;
    expect_success("force stop", &stop)?;
    Ok(())
}
