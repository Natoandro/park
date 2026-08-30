use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-LIFE-003",
    scope = "forceful-stop",
    priority = "P0",
    description = "Force stop a running process without the graceful wait",
    tags = ["lifecycle", "stop", "smoke"]
)]
pub fn force_stop_immediately() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LIFE-003")?;
    let launch = environment.run(&[
        "force-target",
        "--",
        "/bin/sh",
        "-c",
        "trap 'printf term-handler-should-not-run' TERM; sleep 30",
    ])?;
    expect_success("launch", &launch)?;
    expect_success(
        "wait for running",
        &environment.run(&["wait", "force-target", "--state", "running"])? ,
    )?;

    let stop = environment.run(&["stop", "force-target", "--force"])?;
    expect_success("force stop", &stop)?;

    let status = environment.run(&["status", "force-target", "--json"])?;
    expect_success("status", &status)?;
    let value = parse_json("status", &status)?;
    let record = value
        .get("data")
        .ok_or_else(|| "status response is missing data".to_owned())?;
    if record.get("state").and_then(|state| state.as_str()) != Some("killed")
        || record.get("termination_signal").and_then(|signal| signal.as_i64()) != Some(9)
    {
        return Err(format!("force stop has the wrong terminal outcome: {record}"));
    }
    let logs = environment.run(&["logs", "force-target", "--stdout"])?;
    expect_success("force-stop logs", &logs)?;
    if !logs.stdout.is_empty() {
        return Err("force stop allowed the TERM handler to run".to_owned());
    }
    Ok(())
}
