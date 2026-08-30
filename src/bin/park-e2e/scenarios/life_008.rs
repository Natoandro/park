use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{
    TestEnvironment, expect_success, process_is_alive, wait_for_process_exit,
};

#[e2e(
    story = "PARK-LIFE-008",
    scope = "restart-serialization",
    priority = "P0",
    description = "Stop an active generation before starting its replacement",
    tags = ["lifecycle", "restart", "process"]
)]
pub fn restart_active_record_safely() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LIFE-008")?;
    let pid_file = environment.root_path().join("restart-pids");
    let pid_file = pid_file
        .to_str()
        .ok_or_else(|| "restart PID path is not valid UTF-8".to_owned())?;
    let launch = environment.run(&[
        "active-restart",
        "--",
        "/bin/sh",
        "-c",
        "printf '%s\\n' \"$$\" >> \"$1\"; sleep 30",
        "sh",
        pid_file,
    ])?;
    expect_success("launch", &launch)?;
    expect_success(
        "wait for first generation",
        &environment.run(&["wait", "active-restart", "--state", "running"])? ,
    )?;
    let first = environment.run(&["status", "active-restart", "--json"])?;
    expect_success("first status", &first)?;
    let first_pid = serde_json::from_slice::<serde_json::Value>(&first.stdout)
        .map_err(|error| format!("parse first status: {error}"))?
        .get("data")
        .and_then(|data| data.get("pid"))
        .and_then(|pid| pid.as_u64())
        .ok_or_else(|| "first status is missing its PID".to_owned())?;

    let restart = environment.run(&["restart", "active-restart"])?;
    expect_success("restart", &restart)?;
    expect_success(
        "wait for replacement",
        &environment.run(&["wait", "active-restart", "--state", "running"])? ,
    )?;
    if process_is_alive(first_pid as i32) && !wait_for_process_exit(first_pid as i32) {
        return Err("the first process generation remained alive after restart".to_owned());
    }

    let latest = environment.run(&["status", "active-restart", "--json"])?;
    expect_success("latest status", &latest)?;
    let latest = serde_json::from_slice::<serde_json::Value>(&latest.stdout)
        .map_err(|error| format!("parse latest status: {error}"))?;
    let latest_pid = latest
        .get("data")
        .and_then(|data| data.get("pid"))
        .and_then(|pid| pid.as_u64())
        .ok_or_else(|| "latest status is missing its PID".to_owned())?;
    if latest_pid == first_pid
        || latest
            .get("data")
            .and_then(|data| data.get("state"))
            .and_then(|state| state.as_str())
            != Some("running")
    {
        return Err(format!("restart did not create one new active generation: {latest}"));
    }
    expect_success(
        "cleanup",
        &environment.run(&["stop", "active-restart", "--force"])? ,
    )?;
    Ok(())
}
