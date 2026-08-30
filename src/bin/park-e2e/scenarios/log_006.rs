use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success};

#[e2e(
    story = "PARK-LOG-006",
    scope = "tail-slicing",
    priority = "P1",
    description = "Return the last bounded number of retained log lines",
    tags = ["logs", "slicing"]
)]
pub fn return_bounded_log_tail() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LOG-006")?;
    let launch = environment.run(&[
        "tail",
        "--",
        "/bin/sh",
        "-c",
        "printf 'one\\ntwo\\nthree\\n'",
    ])?;
    expect_success("launch", &launch)?;
    expect_success(
        "wait",
        &environment.run(&["wait", "tail", "--exit"] )?,
    )?;

    let last_two = environment.run(&["logs", "tail", "--stdout", "--tail", "2"])?;
    expect_success("tail two", &last_two)?;
    if last_two.stdout != b"two\nthree\n" {
        return Err(format!("tail output differs: {:?}", last_two.stdout));
    }
    let all = environment.run(&["logs", "tail", "--stdout", "--tail", "10"])?;
    expect_success("tail more than available", &all)?;
    if all.stdout != b"one\ntwo\nthree\n" {
        return Err(format!("short tail differs: {:?}", all.stdout));
    }
    let empty = environment.run(&["logs", "tail", "--stdout", "--tail", "0"])?;
    expect_success("tail zero", &empty)?;
    if !empty.stdout.is_empty() {
        return Err("tail zero returned content".to_owned());
    }
    Ok(())
}
