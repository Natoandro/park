use std::time::{Duration, Instant};

use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_contains, expect_exit, expect_success};

#[e2e(
    story = "PARK-WAIT-007",
    scope = "wait-timeouts",
    priority = "P1",
    description = "Honor millisecond, second, and minute timeout units",
    tags = ["wait", "timeouts", "parsing"]
)]
pub fn honor_timeout_units() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-WAIT-007")?;
    let launch = environment.run(&["timeout-target", "--", "/bin/sleep", "30"])?;
    expect_success("launch", &launch)?;

    for unit in ["0ms", "0s", "0m"] {
        let wait = environment.run(&[
            "wait",
            "timeout-target",
            "--state",
            "exited",
            "--timeout",
            unit,
        ])?;
        expect_exit("zero timeout", &wait, 1)?;
        expect_contains(
            &String::from_utf8_lossy(&wait.stderr),
            "timed out waiting for condition",
        )?;
    }

    let started = Instant::now();
    let wait = environment.run(&[
        "wait",
        "timeout-target",
        "--state",
        "exited",
        "--timeout",
        "120ms",
    ])?;
    let elapsed = started.elapsed();
    expect_exit("non-zero timeout", &wait, 1)?;
    expect_contains(
        &String::from_utf8_lossy(&wait.stderr),
        "timed out waiting for condition",
    )?;
    if elapsed < Duration::from_millis(100) || elapsed > Duration::from_secs(2) {
        return Err(format!("120ms wait took {elapsed:?}"));
    }

    let stop = environment.run(&["stop", "timeout-target", "--force"])?;
    expect_success("force stop", &stop)?;
    Ok(())
}
