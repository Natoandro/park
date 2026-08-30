use std::env;
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::process::{Command, Output};

use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-SCOPE-006",
    scope = "unix-arguments",
    priority = "P2",
    description = "Preserve non-UTF-8 names and arguments across restart",
    tags = ["scope", "unix", "arguments"]
)]
pub fn preserve_non_utf8_names_and_arguments() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-SCOPE-006")?;
    let name = OsString::from_vec(b"opaque-\x80-name".to_vec());
    let argument = OsString::from_vec(b"argument-\x80-\xff".to_vec());
    let command = vec![
        OsString::from("/bin/sh"),
        OsString::from("-c"),
        OsString::from("printf '%s' \"$1\" | od -An -tx1"),
        OsString::from("sh"),
        argument.clone(),
    ];

    let mut launch_arguments = vec![name.clone(), OsString::from("--")];
    launch_arguments.extend(command.clone());
    expect_success("non-UTF-8 launch", &run(&environment, &launch_arguments)?)?;

    let wait_arguments = vec![
        OsString::from("wait"),
        name.clone(),
        OsString::from("--exit"),
    ];
    expect_success("wait for non-UTF-8 command", &run(&environment, &wait_arguments)?)?;

    let status_arguments = vec![
        OsString::from("status"),
        name.clone(),
        OsString::from("--json"),
    ];
    let status = run(&environment, &status_arguments)?;
    expect_success("status for non-UTF-8 command", &status)?;
    let record = parse_json("non-UTF-8 status", &status)?
        .get("data")
        .cloned()
        .ok_or_else(|| "non-UTF-8 status has no record".to_owned())?;
    let expected_name = hex(name.as_os_str().as_bytes());
    if record
        .get("key")
        .and_then(|key| key.get("name"))
        .and_then(serde_json::Value::as_str)
        != Some(expected_name.as_str())
    {
        return Err(format!("status lost the non-UTF-8 name: {record}"));
    }
    let expected_arguments = command[1..]
        .iter()
        .map(|argument| hex(argument.as_os_str().as_bytes()))
        .collect::<Vec<_>>();
    let actual_arguments = record
        .get("arguments")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "non-UTF-8 status has no arguments".to_owned())?
        .iter()
        .map(|argument| argument.as_str().unwrap_or_default().to_owned())
        .collect::<Vec<_>>();
    if actual_arguments != expected_arguments {
        return Err(format!(
            "status changed non-UTF-8 arguments: expected {expected_arguments:?}, got {actual_arguments:?}"
        ));
    }

    let logs_arguments = vec![
        OsString::from("logs"),
        name.clone(),
        OsString::from("--stdout"),
    ];
    let first_logs = run(&environment, &logs_arguments)?;
    expect_success("first non-UTF-8 logs", &first_logs)?;
    if !first_logs.stdout.windows(2).any(|bytes| bytes == b"80")
        || !first_logs.stdout.windows(2).any(|bytes| bytes == b"ff")
    {
        return Err(format!(
            "command did not receive the original bytes: {:?}",
            String::from_utf8_lossy(&first_logs.stdout)
        ));
    }

    let restart_arguments = vec![OsString::from("restart"), name.clone()];
    expect_success("restart non-UTF-8 command", &run(&environment, &restart_arguments)?)?;
    expect_success("wait after non-UTF-8 restart", &run(&environment, &wait_arguments)?)?;
    let second_logs = run(&environment, &logs_arguments)?;
    expect_success("second non-UTF-8 logs", &second_logs)?;
    let expected_logs = [first_logs.stdout.as_slice(), first_logs.stdout.as_slice()].concat();
    if second_logs.stdout != expected_logs {
        return Err("restart did not preserve the non-UTF-8 argument".to_owned());
    }
    Ok(())
}

fn run(
    environment: &TestEnvironment,
    arguments: &[OsString],
) -> Result<Output, String> {
    let park = env::var_os("PARK_BIN").ok_or_else(|| "PARK_BIN is not set".to_owned())?;
    let mut command = Command::new(park);
    command
        .args(arguments)
        .current_dir(environment.project_path())
        .env("HOME", environment.root_path().join("home"))
        .env("XDG_STATE_HOME", environment.root_path().join("state"))
        .env("XDG_RUNTIME_DIR", environment.root_path().join("runtime"))
        .env("PARK_E2E_SCENARIO", "PARK-SCOPE-006");
    command
        .output()
        .map_err(|error| format!("execute non-UTF-8 Park command: {error}"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
