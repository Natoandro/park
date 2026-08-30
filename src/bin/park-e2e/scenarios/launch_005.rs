use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success};

const PAYLOAD_SIZE: usize = 131_072;

#[e2e(
    story = "PARK-LAUNCH-005",
    scope = "output-capture",
    priority = "P0",
    description = "Drain high-volume output without deadlock",
    tags = ["launch", "logs", "output", "stress"]
)]
pub fn drain_high_volume_output() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LAUNCH-005")?;
    let launch = environment.run(&[
        "large-output",
        "--",
        "/bin/sh",
        "-c",
        "i=0; while [ \"$i\" -lt 8192 ]; do printf 0123456789abcdef; i=$((i + 1)); done",
    ])?;
    expect_success("launch", &launch)?;
    let wait = environment.run(&["wait", "large-output", "--exit"])?;
    expect_success("wait", &wait)?;

    let logs = environment.run(&["logs", "large-output", "--stdout"])?;
    expect_success("logs", &logs)?;
    if logs.stdout.len() != PAYLOAD_SIZE {
        return Err(format!(
            "large output has {} bytes, expected {PAYLOAD_SIZE}",
            logs.stdout.len()
        ));
    }
    if !logs.stdout.chunks(16).all(|chunk| chunk == b"0123456789abcdef") {
        return Err("large output payload was corrupted".to_owned());
    }
    let status = environment.run(&["status", "large-output", "--json"])?;
    expect_success("status", &status)?;
    if !String::from_utf8_lossy(&status.stdout).contains(r#""state":"exited""#) {
        return Err("large-output record did not reach exited state".to_owned());
    }
    Ok(())
}
