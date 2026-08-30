use std::time::{Duration, Instant};

use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_contains, expect_success};

#[e2e(
    story = "PARK-LAUNCH-001",
    scope = "process-ownership",
    priority = "P0",
    description = "Detach a long-running process from its launching client",
    tags = ["launch", "process", "smoke"]
)]
pub fn detach_long_running_process() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LAUNCH-001")?;
    let started = Instant::now();
    let launch = environment.run(&["detached", "--", "/bin/sleep", "30"])?;
    expect_success("launch", &launch)?;
    if started.elapsed() > Duration::from_secs(2) {
        return Err("launch client did not return promptly".to_owned());
    }

    let status = environment.run(&["status", "detached", "--json"])?;
    expect_success("status from separate client", &status)?;
    expect_contains(
        &String::from_utf8_lossy(&status.stdout),
        r#""state":"running""#,
    )?;
    let stop = environment.run(&["stop", "detached", "--force"])?;
    expect_success("stop", &stop)?;
    Ok(())
}
