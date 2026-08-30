use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-LIFE-007",
    scope = "restart",
    priority = "P0",
    description = "Restart a terminal record from its saved command and append logs",
    tags = ["lifecycle", "restart", "logs"]
)]
pub fn restart_terminal_record() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LIFE-007")?;
    let counter = environment.root_path().join("restart-counter");
    let counter = counter
        .to_str()
        .ok_or_else(|| "restart counter path is not valid UTF-8".to_owned())?;
    let script = "n=$(cat \"$1\" 2>/dev/null || printf 0); n=$((n + 1)); printf 'run-%s\\n' \"$n\"; printf '%s' \"$n\" > \"$1\"";
    let launch = environment.run(&[
        "restart-terminal",
        "--",
        "/bin/sh",
        "-c",
        script,
        "sh",
        counter,
    ])?;
    expect_success("launch", &launch)?;
    expect_success(
        "first wait",
        &environment.run(&["wait", "restart-terminal", "--exit"])? ,
    )?;

    let restart = environment.run(&["restart", "restart-terminal"])?;
    expect_success("restart", &restart)?;
    expect_success(
        "second wait",
        &environment.run(&["wait", "restart-terminal", "--exit"])? ,
    )?;

    let logs = environment.run(&["logs", "restart-terminal", "--stdout"])?;
    expect_success("logs", &logs)?;
    if logs.stdout != b"run-1\nrun-2\n" {
        return Err(format!(
            "restart did not append the second run: {:?}",
            String::from_utf8_lossy(&logs.stdout)
        ));
    }
    let status = environment.run(&["status", "restart-terminal", "--json"])?;
    expect_success("status", &status)?;
    let value = parse_json("status", &status)?;
    if value
        .get("data")
        .and_then(|data| data.get("state"))
        .and_then(|state| state.as_str())
        != Some("exited")
    {
        return Err(format!("restarted record is not terminal: {value}"));
    }
    Ok(())
}
