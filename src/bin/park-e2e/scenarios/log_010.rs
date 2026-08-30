use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-LOG-010",
    scope = "follow-lifecycle",
    priority = "P0",
    description = "End a follow client cleanly when the process terminates",
    tags = ["logs", "follow", "lifecycle"]
)]
pub fn end_follow_when_process_terminates() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LOG-010")?;
    let launch = environment.run(&[
        "follow-exit",
        "--",
        "/bin/sh",
        "-c",
        "printf first; sleep .15; printf second",
    ])?;
    expect_success("launch", &launch)?;

    let followed = environment.run(&["logs", "follow-exit", "--follow", "--stdout"])?;
    expect_success("follow logs", &followed)?;
    if followed.stdout != b"firstsecond" {
        return Err(format!("follow did not deliver all output: {:?}", followed.stdout));
    }
    let status = environment.run(&["status", "follow-exit", "--json"])?;
    expect_success("final status", &status)?;
    let value = parse_json("final status", &status)?;
    if value["data"]["state"] != "exited" {
        return Err(format!("followed record was not terminal: {value}"));
    }
    Ok(())
}
