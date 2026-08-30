use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success};

#[e2e(
    story = "PARK-LOG-001",
    scope = "combined-logs",
    priority = "P0",
    description = "Read retained stdout followed by stderr as one log view",
    tags = ["logs", "output"]
)]
pub fn read_combined_logs() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LOG-001")?;
    let launch = environment.run(&[
        "combined",
        "--",
        "/bin/sh",
        "-c",
        "printf 'out-one\\nout-two\\n'; printf 'err-one\\nerr-two\\n' >&2",
    ])?;
    expect_success("launch", &launch)?;
    expect_success(
        "wait",
        &environment.run(&["wait", "combined", "--exit"] )?,
    )?;

    let logs = environment.run(&["logs", "combined"])?;
    expect_success("combined logs", &logs)?;
    if logs.stdout != b"out-one\nout-two\nerr-one\nerr-two\n" {
        return Err(format!("combined log differs: {:?}", logs.stdout));
    }
    Ok(())
}
