use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-DAEMON-007",
    scope = "persistence",
    priority = "P0",
    description = "Retain terminal records and both log streams after exit",
    tags = ["daemon", "persistence", "logs"]
)]
pub fn retain_records_after_normal_process_exit() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-DAEMON-007")?;
    let launch = environment.run(&[
        "retained-exit",
        "--",
        "/bin/sh",
        "-c",
        "printf retained-out; printf retained-err >&2; exit 7",
    ])?;
    expect_success("launch", &launch)?;

    let wait = environment.run(&["wait", "retained-exit", "--exit"])?;
    expect_success("wait from separate client", &wait)?;

    let status = environment.run(&["status", "retained-exit", "--json"])?;
    expect_success("status after exit", &status)?;
    let status_json = parse_json("status after exit", &status)?;
    let record = status_json
        .get("data")
        .ok_or_else(|| "status after exit is missing its record".to_owned())?;
    if record.get("state").and_then(|value| value.as_str()) != Some("exited")
        || record.get("exit_code").and_then(|value| value.as_i64()) != Some(7)
        || record.get("exited_at").and_then(|value| value.as_u64()).is_none()
    {
        return Err(format!("terminal record was not retained: {record}"));
    }

    let stdout = environment.run(&["logs", "retained-exit", "--stdout"])?;
    expect_success("retained stdout", &stdout)?;
    if stdout.stdout != b"retained-out" {
        return Err(format!("retained stdout differs: {:?}", stdout.stdout));
    }
    let stderr = environment.run(&["logs", "retained-exit", "--stderr"])?;
    expect_success("retained stderr", &stderr)?;
    if stderr.stdout != b"retained-err" {
        return Err(format!("retained stderr differs: {:?}", stderr.stdout));
    }
    Ok(())
}
