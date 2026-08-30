use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success};

#[e2e(
    story = "PARK-LOG-003",
    scope = "stderr-selection",
    priority = "P0",
    description = "Return only stderr from a record with two distinct streams",
    tags = ["logs", "stderr"]
)]
pub fn select_only_stderr() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LOG-003")?;
    let launch = environment.run(&[
        "stderr-selection",
        "--",
        "/bin/sh",
        "-c",
        "printf stdout-line; printf stderr-line >&2",
    ])?;
    expect_success("launch", &launch)?;
    expect_success(
        "wait",
        &environment.run(&["wait", "stderr-selection", "--exit"] )?,
    )?;

    let logs = environment.run(&["logs", "stderr-selection", "--stderr"])?;
    expect_success("stderr logs", &logs)?;
    if logs.stdout != b"stderr-line" {
        return Err(format!("stderr log differs: {:?}", logs.stdout));
    }
    Ok(())
}
