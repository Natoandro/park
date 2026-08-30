use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{
    TestEnvironment, expect_exit, expect_stderr_nonempty, expect_success,
};

#[e2e(
    story = "PARK-LOG-004",
    scope = "log-option-validation",
    priority = "P1",
    description = "Reject simultaneous stdout and stderr log selectors",
    tags = ["logs", "cli", "errors"]
)]
pub fn reject_conflicting_stream_selectors() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LOG-004")?;
    let launch = environment.run(&["conflict", "--", "/bin/true"])?;
    expect_success("launch", &launch)?;
    expect_success(
        "wait",
        &environment.run(&["wait", "conflict", "--exit"] )?,
    )?;

    let logs = environment.run(&["logs", "conflict", "--stdout", "--stderr"])?;
    expect_exit("conflicting log selectors", &logs, 2)?;
    expect_stderr_nonempty("conflicting log selectors", &logs)?;
    if !String::from_utf8_lossy(&logs.stderr).contains("cannot be used") {
        return Err(format!(
            "conflict diagnostic is unclear: {}",
            String::from_utf8_lossy(&logs.stderr)
        ));
    }
    if !logs.stdout.is_empty() {
        return Err("conflicting selector wrote stdout".to_owned());
    }
    Ok(())
}
