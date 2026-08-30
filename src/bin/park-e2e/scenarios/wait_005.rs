use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-WAIT-005",
    scope = "wait-restart-history",
    priority = "P1",
    description = "Match output appended by a later restart",
    tags = ["wait", "logs", "restart"]
)]
pub fn match_output_after_restart() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-WAIT-005")?;
    let marker_path = environment.root_path().join("restart-marker");
    let marker_path = marker_path
        .to_str()
        .ok_or_else(|| "restart marker path is not valid UTF-8".to_owned())?;
    let script = "if [ -e \"$1\" ]; then printf later[.*]; printf later-error[?] >&2; else : > \"$1\"; printf first; printf first-error[?] >&2; fi";
    let launch = environment.run(&[
        "restart-history",
        "--",
        "/bin/sh",
        "-c",
        script,
        "sh",
        marker_path,
    ])?;
    expect_success("initial launch", &launch)?;
    expect_success(
        "initial wait",
        &environment.run(&["wait", "restart-history", "--exit"] )?,
    )?;

    let restart = environment.run(&["restart", "restart-history"])?;
    expect_success("restart", &restart)?;
    expect_success(
        "restart wait",
        &environment.run(&["wait", "restart-history", "--exit"] )?,
    )?;

    let stdout_match = environment.run(&[
        "wait",
        "restart-history",
        "--match",
        "later[.*]",
        "--timeout",
        "1s",
    ])?;
    expect_success("later stdout match", &stdout_match)?;
    let stderr_match = environment.run(&[
        "wait",
        "restart-history",
        "--match",
        "later-error[?]",
        "--timeout",
        "1s",
    ])?;
    expect_success("later stderr match", &stderr_match)?;

    let stdout = environment.run(&["logs", "restart-history", "--stdout"])?;
    expect_success("retained stdout", &stdout)?;
    if stdout.stdout != b"firstlater[.*]" {
        return Err(format!("retained stdout is wrong: {:?}", stdout.stdout));
    }
    let stderr = environment.run(&["logs", "restart-history", "--stderr"])?;
    expect_success("retained stderr", &stderr)?;
    if stderr.stdout != b"first-error[?]later-error[?]" {
        return Err(format!("retained stderr is wrong: {:?}", stderr.stdout));
    }
    let record = parse_json("later stdout match", &stdout_match)?;
    if record.get("state").and_then(|value| value.as_str()) != Some("exited") {
        return Err(format!("restart match returned the wrong record: {record}"));
    }
    Ok(())
}
