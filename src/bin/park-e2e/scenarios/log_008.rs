use std::fs;

use park_e2e_macros::e2e;
use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-LOG-008",
    scope = "log-rendering",
    priority = "P2",
    description = "Handle empty logs and retain invalid bytes without crashing",
    tags = ["logs", "json", "edge"]
)]
pub fn handle_empty_and_invalid_byte_logs() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LOG-008")?;

    let empty_launch = environment.run(&["empty-log", "--", "/bin/true"])?;
    expect_success("empty launch", &empty_launch)?;
    expect_success(
        "empty wait",
        &environment.run(&["wait", "empty-log", "--exit"] )?,
    )?;
    let empty = environment.run(&["logs", "empty-log"])?;
    expect_success("empty human logs", &empty)?;
    if !empty.stdout.is_empty() {
        return Err("empty logs returned content".to_owned());
    }
    let empty_json = environment.run(&["logs", "empty-log", "--json"])?;
    expect_success("empty JSON logs", &empty_json)?;
    let empty_value = parse_json("empty JSON logs", &empty_json)?;
    if empty_value["data"]["content"] != "" {
        return Err(format!("empty JSON content was not empty: {empty_value}"));
    }

    let invalid_launch = environment.run(&[
        "invalid-log",
        "--",
        "/bin/sh",
        "-c",
        "printf '\\377\\n'",
    ])?;
    expect_success("invalid-byte launch", &invalid_launch)?;
    let status = environment.run(&["status", "invalid-log", "--json"])?;
    expect_success("invalid-byte status", &status)?;
    let status_value = parse_json("invalid-byte status", &status)?;
    let stdout_path = status_value["data"]["logs"]["stdout"]
        .as_str()
        .ok_or_else(|| "status record did not contain a stdout log path".to_owned())?;
    expect_success(
        "invalid-byte wait",
        &environment.run(&["wait", "invalid-log", "--exit"] )?,
    )?;
    if fs::read(stdout_path).map_err(|error| error.to_string())? != [0xff, b'\n'] {
        return Err("invalid byte was not retained on disk".to_owned());
    }

    let human = environment.run(&["logs", "invalid-log", "--stdout"])?;
    expect_success("invalid human logs", &human)?;
    if human.stdout != "\u{fffd}\n".as_bytes() {
        return Err(format!("invalid human rendering differs: {:?}", human.stdout));
    }
    let json = environment.run(&["logs", "invalid-log", "--stdout", "--json"])?;
    expect_success("invalid JSON logs", &json)?;
    let value = parse_json("invalid JSON logs", &json)?;
    if value["data"]["content"] != "\u{fffd}\n" {
        return Err(format!("invalid JSON rendering differs: {value}"));
    }
    Ok(())
}
