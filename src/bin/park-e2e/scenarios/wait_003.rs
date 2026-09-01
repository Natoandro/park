use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_contains, expect_success};

#[e2e(
    story = "PARK-WAIT-003",
    scope = "wait-output",
    priority = "P0",
    description = "Match literal readiness output in either stream",
    tags = ["wait", "logs", "output"]
)]
pub fn wait_for_literal_output_in_either_stream() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-WAIT-003")?;
    let launch = environment.run(&[
        "output-target",
        "--",
        "/bin/sh",
        "-c",
        "sleep 0.1; printf 'ready[.*]'; printf 'stderr(?)' >&2; sleep 30",
    ])?;
    expect_success("launch", &launch)?;

    let stdout_match = environment.run(&[
        "wait",
        "output-target",
        "--match",
        "ready[.*]",
        "--timeout",
        "2s",
    ])?;
    expect_success("stdout match", &stdout_match)?;
    expect_contains(&String::from_utf8_lossy(&stdout_match.stdout), "State: running")?;
    expect_contains(&String::from_utf8_lossy(&stdout_match.stdout), "PID: ")?;

    let stderr_match = environment.run(&[
        "wait",
        "output-target",
        "--match",
        "stderr(?)",
        "--timeout",
        "2s",
    ])?;
    expect_success("stderr match", &stderr_match)?;
    expect_contains(&String::from_utf8_lossy(&stderr_match.stdout), "State: running")?;

    let stop = environment.run(&["stop", "output-target", "--force"])?;
    expect_success("force stop", &stop)?;
    Ok(())
}
