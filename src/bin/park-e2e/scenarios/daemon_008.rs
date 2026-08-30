use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-DAEMON-008",
    scope = "persistence-recovery",
    priority = "P2",
    description = "Recreate a stale log-only artifact on a later launch",
    tags = ["daemon", "persistence", "recovery", "logs"]
)]
pub fn recover_interrupted_pre_spawn_log_artifact() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-DAEMON-008")?;

    let stale = environment.run(&[
        "stale-key",
        "--",
        "/bin/sh",
        "-c",
        "printf stale-output",
    ])?;
    expect_success("initial stale-key launch", &stale)?;
    let wait = environment.run(&["wait", "stale-key", "--exit"])?;
    expect_success("wait for initial stale-key launch", &wait)?;
    expect_success(
        "remove initial stale-key record",
        &environment.run(&["rm", "stale-key", "--keep-logs"])?,
    )?;

    let unrelated = environment.run(&[
        "unrelated-key",
        "--",
        "/bin/sh",
        "-c",
        "printf unrelated-output",
    ])?;
    expect_success("unrelated launch", &unrelated)?;
    let wait = environment.run(&["wait", "unrelated-key", "--exit"])?;
    expect_success("wait for unrelated launch", &wait)?;

    let retry = environment.run(&[
        "stale-key",
        "--",
        "/bin/sh",
        "-c",
        "printf fresh-output",
    ])?;
    expect_success("retry stale-key launch", &retry)?;
    let wait = environment.run(&["wait", "stale-key", "--exit"])?;
    expect_success("wait for retry", &wait)?;

    let fresh_logs = environment.run(&["logs", "stale-key", "--stdout"])?;
    expect_success("recreated stale-key logs", &fresh_logs)?;
    if fresh_logs.stdout != b"fresh-output" {
        return Err(format!(
            "recreated logs contain stale data: {:?}",
            fresh_logs.stdout
        ));
    }
    let unrelated_logs = environment.run(&["logs", "unrelated-key", "--stdout"])?;
    expect_success("unrelated logs", &unrelated_logs)?;
    if unrelated_logs.stdout != b"unrelated-output" {
        return Err(format!(
            "unrelated logs changed: {:?}",
            unrelated_logs.stdout
        ));
    }
    let status = environment.run(&["status", "stale-key", "--json"])?;
    expect_success("recreated record status", &status)?;
    let status_json = parse_json("recreated record status", &status)?;
    if status_json
        .get("data")
        .and_then(|data| data.get("state"))
        .and_then(|state| state.as_str())
        != Some("exited")
    {
        return Err(format!("recreated record is not terminal: {status_json}"));
    }
    Ok(())
}
