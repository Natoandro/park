use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-WAIT-003",
    scope = "wait-output",
    priority = "P0",
    description = "Match literal readiness output in either stream",
    tags = ["wait", "logs", "output"]
)]
pub fn wait_for_literal_output_in_either_stream() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-WAIT-003")?;
    let launch = environment.run(&[
        "output-target",
        "--",
        "/bin/sh",
        "-c",
        "sleep 0.1; printf 'ready[.*]'; printf 'stderr(?)' >&2; sleep 30",
    ])?;
    expect_success("launch", &launch)?;

    let stdout_match = environment.run(&[
        "wait",
        "output-target",
        "--match",
        "ready[.*]",
        "--timeout",
        "2s",
    ])?;
    expect_success("stdout match", &stdout_match)?;
    let stdout_record = parse_json("stdout match", &stdout_match)?;
    if stdout_record.get("state").and_then(|value| value.as_str()) != Some("running")
        || stdout_record.get("pid").and_then(|value| value.as_u64()).is_none()
    {
        return Err(format!("stdout match returned a stale record: {stdout_record}"));
    }

    let stderr_match = environment.run(&[
        "wait",
        "output-target",
        "--match",
        "stderr(?)",
        "--timeout",
        "2s",
    ])?;
    expect_success("stderr match", &stderr_match)?;
    let stderr_record = parse_json("stderr match", &stderr_match)?;
    if stderr_record.get("state").and_then(|value| value.as_str()) != Some("running") {
        return Err(format!("stderr match returned the wrong record: {stderr_record}"));
    }

    let stop = environment.run(&["stop", "output-target", "--force"])?;
    expect_success("force stop", &stop)?;
    Ok(())
}
