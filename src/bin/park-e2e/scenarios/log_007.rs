use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success};

#[e2e(
    story = "PARK-LOG-007",
    scope = "grep-and-slicing",
    priority = "P1",
    description = "Apply literal grep before selecting a log head or tail",
    tags = ["logs", "grep", "slicing"]
)]
pub fn filter_logs_before_slicing() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LOG-007")?;
    let launch = environment.run(&[
        "grep-order",
        "--",
        "/bin/sh",
        "-c",
        "printf 'a.b first\\naxb second\\na.b last\\n'",
    ])?;
    expect_success("launch", &launch)?;
    expect_success(
        "wait",
        &environment.run(&["wait", "grep-order", "--exit"] )?,
    )?;

    let head = environment.run(&["logs", "grep-order", "--grep", "a.b", "--head", "1"])?;
    expect_success("grep head", &head)?;
    if head.stdout != b"a.b first\n" {
        return Err(format!("grep head differs: {:?}", head.stdout));
    }
    let tail = environment.run(&["logs", "grep-order", "--grep", "a.b", "--tail", "1"])?;
    expect_success("grep tail", &tail)?;
    if tail.stdout != b"a.b last\n" {
        return Err(format!("grep tail differs: {:?}", tail.stdout));
    }
    Ok(())
}
