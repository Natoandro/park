use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success};

#[e2e(
    story = "PARK-CLI-006",
    scope = "parsing",
    priority = "P0",
    description = "Pass command flags after the launch separator",
    tags = ["cli", "parsing", "smoke"]
)]
pub fn pass_command_flags_after_separator() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-CLI-006")?;

    let launch = environment.run(&[
        "flags",
        "--",
        "/bin/sh",
        "-c",
        "printf '%s' \"$1\"",
        "sh",
        "--child-flag",
    ])?;
    expect_success("launch", &launch)?;
    let wait = environment.run(&["wait", "flags", "--exit"])?;
    expect_success("wait", &wait)?;

    let logs = environment.run(&["logs", "flags", "--stdout"])?;
    expect_success("logs", &logs)?;
    if logs.stdout != b"--child-flag" {
        return Err(format!(
            "child did not receive its flag: {:?}",
            String::from_utf8_lossy(&logs.stdout)
        ));
    }
    Ok(())
}
