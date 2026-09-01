use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_contains, expect_exit, expect_success};

#[e2e(
    story = "PARK-WAIT-001",
    scope = "wait-state",
    priority = "P0",
    description = "Wait for an exact lifecycle state",
    tags = ["wait", "lifecycle", "smoke"]
)]
pub fn wait_for_exact_state() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-WAIT-001")?;
    let launch = environment.run(&["state-target", "--", "/bin/sleep", "2"])?;
    expect_success("launch", &launch)?;

    let running = environment.run(&["wait", "state-target", "--state", "running"])?;
    expect_success("wait for running", &running)?;
    expect_contains(&String::from_utf8_lossy(&running.stdout), "State: running")?;
    expect_contains(&String::from_utf8_lossy(&running.stdout), "PID: ")?;

    let wrong_state = environment.run(&[
        "wait",
        "state-target",
        "--state",
        "failed",
        "--timeout",
        "0ms",
    ])?;
    expect_exit("wrong state wait", &wrong_state, 1)?;
    expect_contains(
        &String::from_utf8_lossy(&wrong_state.stderr),
        "timed out waiting for condition",
    )?;

    let exited = environment.run(&["wait", "state-target", "--state", "exited"])?;
    expect_success("wait for exited", &exited)?;
    expect_contains(&String::from_utf8_lossy(&exited.stdout), "State: exited")?;
    expect_contains(&String::from_utf8_lossy(&exited.stdout), "Exit code: 0")?;
    Ok(())
}
