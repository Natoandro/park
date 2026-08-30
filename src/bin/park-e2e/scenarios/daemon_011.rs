use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-DAEMON-011",
    scope = "persistence-reconnect",
    priority = "P0",
    description = "Expose the same record and logs to separate Park clients",
    tags = ["daemon", "persistence", "reconnect"]
)]
pub fn preserve_state_across_client_reconnects() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-DAEMON-011")?;
    let launch = environment.run(&[
        "reconnectable",
        "--",
        "/bin/sh",
        "-c",
        "printf reconnect-marker; sleep 30",
    ])?;
    expect_success("launch client", &launch)?;
    let launch_record = parse_json("launch client", &launch)?;

    let status = environment.run(&["status", "reconnectable", "--json"])?;
    expect_success("status client", &status)?;
    let status_json = parse_json("status client", &status)?;
    let status_record = status_json
        .get("data")
        .ok_or_else(|| "status client is missing its record".to_owned())?;
    for field in [
        "key",
        "working_directory",
        "executable",
        "arguments",
        "state",
        "logs",
    ] {
        if launch_record.get(field) != status_record.get(field) {
            return Err(format!(
                "reconnected status changed {field}: launch={}, status={}",
                launch_record.get(field).unwrap_or(&serde_json::Value::Null),
                status_record.get(field).unwrap_or(&serde_json::Value::Null)
            ));
        }
    }

    let logs = environment.run(&["logs", "reconnectable", "--stdout"])?;
    expect_success("logs client", &logs)?;
    if logs.stdout != b"reconnect-marker" {
        return Err(format!("reconnected client saw unexpected logs: {:?}", logs.stdout));
    }

    let records = environment.run(&["ps", "--json"])?;
    expect_success("registry client", &records)?;
    let records_json = parse_json("registry client", &records)?;
    if records_json
        .get("data")
        .and_then(|data| data.as_array())
        .is_none_or(|records| records.len() != 1)
    {
        return Err(format!("reconnect created an unexpected registry: {records_json}"));
    }

    let stop = environment.run(&["stop", "reconnectable", "--force"])?;
    expect_success("control client", &stop)?;
    let wait = environment.run(&["wait", "reconnectable", "--exit"])?;
    expect_success("wait after control", &wait)?;
    let final_status = environment.run(&["status", "reconnectable", "--json"])?;
    expect_success("final status client", &final_status)?;
    let final_json = parse_json("final status client", &final_status)?;
    if final_json
        .get("data")
        .and_then(|data| data.get("state"))
        .and_then(|state| state.as_str())
        != Some("killed")
    {
        return Err(format!("controlled record did not reach killed state: {final_json}"));
    }
    Ok(())
}
