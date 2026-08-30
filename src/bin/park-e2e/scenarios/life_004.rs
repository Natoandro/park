use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_exit, expect_success, parse_json};

#[e2e(
    story = "PARK-LIFE-004",
    scope = "invalid-lifecycle-state",
    priority = "P0",
    description = "Reject stop for exited, failed, and killed records",
    tags = ["lifecycle", "errors"]
)]
pub fn reject_stop_for_terminal_records() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LIFE-004")?;

    let launch = environment.run(&[
        "exited-record",
        "--",
        "/bin/sh",
        "-c",
        "printf terminal; exit 7",
    ])?;
    expect_success("exited launch", &launch)?;
    expect_success(
        "wait for exited record",
        &environment.run(&["wait", "exited-record", "--exit"])? ,
    )?;
    reject_stop(&environment, "exited-record")?;

    let failed = environment.run(&["failed-record", "--", "/missing/park-e2e-command"])?;
    expect_exit("failed launch", &failed, 1)?;
    reject_stop(&environment, "failed-record")?;

    let killed = environment.run(&["killed-record", "--", "/bin/sleep", "30"])?;
    expect_success("killed launch", &killed)?;
    expect_success(
        "wait for killed target",
        &environment.run(&["wait", "killed-record", "--state", "running"])? ,
    )?;
    expect_success(
        "kill target",
        &environment.run(&["stop", "killed-record", "--force"])? ,
    )?;
    reject_stop(&environment, "killed-record")
}

fn reject_stop(environment: &TestEnvironment, name: &str) -> Result<(), String> {
    let status_before = environment.run(&["status", name, "--json"])?;
    expect_success("status before invalid stop", &status_before)?;
    let status_before = parse_json("status before invalid stop", &status_before)?;
    let logs_before = environment.run(&["logs", name, "--stdout"])?;
    expect_success("logs before invalid stop", &logs_before)?;

    let stop = environment.run(&["stop", name])?;
    expect_exit("invalid terminal stop", &stop, 5)?;
    let status_after = environment.run(&["status", name, "--json"])?;
    expect_success("status after invalid stop", &status_after)?;
    if status_before != parse_json("status after invalid stop", &status_after)? {
        return Err(format!("invalid stop changed the {name} record"));
    }
    let logs_after = environment.run(&["logs", name, "--stdout"])?;
    expect_success("logs after invalid stop", &logs_after)?;
    if logs_before.stdout != logs_after.stdout {
        return Err(format!("invalid stop changed the {name} logs"));
    }
    Ok(())
}
