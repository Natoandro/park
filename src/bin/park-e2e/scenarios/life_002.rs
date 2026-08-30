use std::time::{Duration, Instant};

use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-LIFE-002",
    scope = "stop-timeout",
    priority = "P0",
    description = "Escalate a process that ignores graceful termination",
    tags = ["lifecycle", "stop", "stress"]
)]
pub fn escalate_a_stubborn_process() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LIFE-002")?;
    let launch = environment.run(&[
        "stubborn",
        "--",
        "/bin/sh",
        "-c",
        "trap '' TERM; sleep 30",
    ])?;
    expect_success("launch", &launch)?;
    expect_success(
        "wait for running",
        &environment.run(&["wait", "stubborn", "--state", "running"])? ,
    )?;

    let started = Instant::now();
    let stop = environment.run(&["stop", "stubborn"])?;
    expect_success("stop", &stop)?;
    let elapsed = started.elapsed();
    if elapsed < Duration::from_millis(1_500) || elapsed > Duration::from_secs(6) {
        return Err(format!("stop escalation took {elapsed:?}, expected about two seconds"));
    }

    let status = environment.run(&["status", "stubborn", "--json"])?;
    expect_success("status", &status)?;
    let value = parse_json("status", &status)?;
    let record = value
        .get("data")
        .ok_or_else(|| "status response is missing data".to_owned())?;
    if record.get("state").and_then(|state| state.as_str()) != Some("killed")
        || record
            .get("termination_signal")
            .and_then(|signal| signal.as_i64())
            .is_none_or(|signal| signal <= 0)
    {
        return Err(format!("stubborn process was not killed after escalation: {record}"));
    }
    Ok(())
}
