use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success};

#[e2e(
    story = "PARK-LAUNCH-003",
    scope = "stdout-capture",
    priority = "P0",
    description = "Capture stdout separately and retain it after exit",
    tags = ["launch", "logs", "output"]
)]
pub fn capture_stdout_separately() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LAUNCH-003")?;

    let launch = environment.run(&[
        "stdout-only",
        "--",
        "/bin/sh",
        "-c",
        "printf stdout-marker",
    ])?;
    expect_success("launch", &launch)?;
    let wait = environment.run(&["wait", "stdout-only", "--exit"])?;
    expect_success("wait", &wait)?;

    let stdout = environment.run(&["logs", "stdout-only", "--stdout"])?;
    expect_success("stdout logs", &stdout)?;
    if stdout.stdout != b"stdout-marker" {
        return Err(format!(
            "stdout log differs: {:?}",
            String::from_utf8_lossy(&stdout.stdout)
        ));
    }
    let stderr = environment.run(&["logs", "stdout-only", "--stderr"])?;
    expect_success("stderr logs", &stderr)?;
    if !stderr.stdout.is_empty() {
        return Err("stdout-only command wrote unexpected stderr".to_owned());
    }
    let status = environment.run(&["status", "stdout-only", "--json"])?;
    expect_success("status", &status)?;
    if !String::from_utf8_lossy(&status.stdout).contains(r#""state":"exited""#) {
        return Err("stdout record did not reach exited state".to_owned());
    }
    Ok(())
}
