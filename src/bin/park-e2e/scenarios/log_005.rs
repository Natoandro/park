use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success};

#[e2e(
    story = "PARK-LOG-005",
    scope = "head-slicing",
    priority = "P1",
    description = "Return the first bounded number of retained log lines",
    tags = ["logs", "slicing"]
)]
pub fn return_bounded_log_head() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LOG-005")?;
    let launch = environment.run(&[
        "head",
        "--",
        "/bin/sh",
        "-c",
        "printf 'one\\ntwo\\nthree\\n'",
    ])?;
    expect_success("launch", &launch)?;
    expect_success(
        "wait",
        &environment.run(&["wait", "head", "--exit"] )?,
    )?;

    let first_two = environment.run(&["logs", "head", "--stdout", "--head", "2"])?;
    expect_success("head two", &first_two)?;
    if first_two.stdout != b"one\ntwo\n" {
        return Err(format!("head output differs: {:?}", first_two.stdout));
    }
    let empty = environment.run(&["logs", "head", "--stdout", "--head", "0"])?;
    expect_success("head zero", &empty)?;
    if !empty.stdout.is_empty() {
        return Err("head zero returned content".to_owned());
    }
    Ok(())
}
