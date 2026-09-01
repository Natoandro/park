use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_exit, expect_success, expect_contains};

#[e2e(
    story = "PARK-WAIT-002",
    scope = "wait-terminal",
    priority = "P0",
    description = "Wait for every terminal process outcome",
    tags = ["wait", "lifecycle", "terminal"]
)]
pub fn wait_for_any_terminal_exit() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-WAIT-002")?;

    let exited_launch = environment.run(&["exited", "--", "/bin/true"])?;
    expect_success("exited launch", &exited_launch)?;
    let exited_wait = environment.run(&["wait", "exited", "--exit"])?;
    expect_success("exited wait", &exited_wait)?;
    expect_contains(&String::from_utf8_lossy(&exited_wait.stdout), "State: exited")?;

    let failed_launch = environment.run(&["failed", "--", "/definitely/missing/park-command"])?;
    expect_exit("failed launch", &failed_launch, 1)?;
    let failed_wait = environment.run(&["wait", "failed", "--exit"])?;
    expect_success("failed wait", &failed_wait)?;
    expect_contains(&String::from_utf8_lossy(&failed_wait.stdout), "State: failed")?;

    let killed_launch = environment.run(&["killed", "--", "/bin/sleep", "30"])?;
    expect_success("killed launch", &killed_launch)?;
    let premature = environment.run(&["wait", "killed", "--exit", "--timeout", "0ms"])?;
    expect_exit("premature terminal wait", &premature, 1)?;
    let stop = environment.run(&["stop", "killed", "--force"])?;
    expect_success("force stop", &stop)?;
    let killed_wait = environment.run(&["wait", "killed", "--exit"])?;
    expect_success("killed wait", &killed_wait)?;
    expect_contains(&String::from_utf8_lossy(&killed_wait.stdout), "State: killed")?;
    expect_contains(&String::from_utf8_lossy(&killed_wait.stdout), "Termination signal: ")?;
    Ok(())
}
