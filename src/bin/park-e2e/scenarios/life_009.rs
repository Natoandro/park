use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_exit, expect_success, parse_json};

#[e2e(
    story = "PARK-LIFE-009",
    scope = "start",
    priority = "P1",
    description = "Start a terminal record and reject start while it is active",
    tags = ["lifecycle", "start"]
)]
pub fn start_terminal_record_only() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LIFE-009")?;
    let launch = environment.run(&[
        "start-target",
        "--",
        "/bin/sh",
        "-c",
        "printf start-marker; sleep 30",
    ])?;
    expect_success("launch", &launch)?;
    expect_success(
        "wait for first run",
        &environment.run(&["wait", "start-target", "--state", "running"])? ,
    )?;
    expect_success(
        "stop first run",
        &environment.run(&["stop", "start-target", "--force"])? ,
    )?;

    let start = environment.run(&["start", "start-target"])?;
    expect_success("start terminal record", &start)?;
    expect_success(
        "wait for active start",
        &environment.run(&["wait", "start-target", "--state", "running"])? ,
    )?;
    let duplicate_start = environment.run(&["start", "start-target"])?;
    expect_exit("start active record", &duplicate_start, 5)?;

    let status = environment.run(&["status", "start-target", "--json"])?;
    expect_success("active status", &status)?;
    let value = parse_json("active status", &status)?;
    if value
        .get("data")
        .and_then(|data| data.get("state"))
        .and_then(|state| state.as_str())
        != Some("running")
    {
        return Err(format!("start active refusal did not preserve the process: {value}"));
    }
    expect_success(
        "cleanup",
        &environment.run(&["stop", "start-target", "--force"])? ,
    )?;
    let logs = environment.run(&["logs", "start-target", "--stdout"])?;
    expect_success("logs", &logs)?;
    if logs.stdout.iter().filter(|byte| **byte == b's').count() < 2 {
        return Err("start did not append the second run's output".to_owned());
    }
    Ok(())
}
