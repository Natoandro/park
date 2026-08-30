use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success};

#[e2e(
    story = "PARK-LOG-002",
    scope = "stdout-selection",
    priority = "P0",
    description = "Return only stdout from a record with two distinct streams",
    tags = ["logs", "stdout"]
)]
pub fn select_only_stdout() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LOG-002")?;
    let launch = environment.run(&[
        "stdout-selection",
        "--",
        "/bin/sh",
        "-c",
        "printf stdout-line; printf stderr-line >&2",
    ])?;
    expect_success("launch", &launch)?;
    expect_success(
        "wait",
        &environment.run(&["wait", "stdout-selection", "--exit"] )?,
    )?;

    let logs = environment.run(&["logs", "stdout-selection", "--stdout"])?;
    expect_success("stdout logs", &logs)?;
    if logs.stdout != b"stdout-line" {
        return Err(format!("stdout log differs: {:?}", logs.stdout));
    }
    if !logs.stderr.is_empty() {
        return Err("stdout log request wrote a diagnostic".to_owned());
    }
    Ok(())
}
