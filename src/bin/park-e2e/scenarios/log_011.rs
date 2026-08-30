use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-LOG-011",
    scope = "restart-log-retention",
    priority = "P0",
    description = "Append stdout and stderr history across restart and start",
    tags = ["logs", "lifecycle", "retention"]
)]
pub fn preserve_logs_across_restart_and_start() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LOG-011")?;
    let command = "n=$(cat count 2>/dev/null || printf 0); n=$((n + 1)); printf \"run$n\"; printf \"err$n\" >&2; printf \"%s\" \"$n\" > count";
    let launch = environment.run(&["append-history", "--", "/bin/sh", "-c", command])?;
    expect_success("launch", &launch)?;
    expect_success(
        "first wait",
        &environment.run(&["wait", "append-history", "--exit"] )?,
    )?;
    let restart = environment.run(&["restart", "append-history"])?;
    expect_success("restart", &restart)?;
    expect_success(
        "second wait",
        &environment.run(&["wait", "append-history", "--exit"] )?,
    )?;
    let start = environment.run(&["start", "append-history"])?;
    expect_success("start", &start)?;
    expect_success(
        "third wait",
        &environment.run(&["wait", "append-history", "--exit"] )?,
    )?;

    let stdout = environment.run(&["logs", "append-history", "--stdout"])?;
    expect_success("stdout history", &stdout)?;
    if stdout.stdout != b"run1run2run3" {
        return Err(format!("stdout history differs: {:?}", stdout.stdout));
    }
    let stderr = environment.run(&["logs", "append-history", "--stderr"])?;
    expect_success("stderr history", &stderr)?;
    if stderr.stdout != b"err1err2err3" {
        return Err(format!("stderr history differs: {:?}", stderr.stdout));
    }
    let status = environment.run(&["status", "append-history", "--json"])?;
    expect_success("latest status", &status)?;
    if parse_json("latest status", &status)?["data"]["state"] != "exited" {
        return Err("latest run was not terminal".to_owned());
    }
    Ok(())
}
