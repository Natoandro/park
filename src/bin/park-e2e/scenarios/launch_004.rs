use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success};

#[e2e(
    story = "PARK-LAUNCH-004",
    scope = "stderr-capture",
    priority = "P0",
    description = "Capture stderr separately and retain it after exit",
    tags = ["launch", "logs", "output"]
)]
pub fn capture_stderr_separately() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LAUNCH-004")?;

    let launch = environment.run(&[
        "stderr-only",
        "--",
        "/bin/sh",
        "-c",
        "printf stderr-marker >&2",
    ])?;
    expect_success("launch", &launch)?;
    let wait = environment.run(&["wait", "stderr-only", "--exit"])?;
    expect_success("wait", &wait)?;

    let stderr = environment.run(&["logs", "stderr-only", "--stderr"])?;
    expect_success("stderr logs", &stderr)?;
    if stderr.stdout != b"stderr-marker" {
        return Err(format!(
            "stderr log differs: {:?}",
            String::from_utf8_lossy(&stderr.stdout)
        ));
    }
    let stdout = environment.run(&["logs", "stderr-only", "--stdout"])?;
    expect_success("stdout logs", &stdout)?;
    if !stdout.stdout.is_empty() {
        return Err("stderr-only command wrote unexpected stdout".to_owned());
    }
    Ok(())
}
