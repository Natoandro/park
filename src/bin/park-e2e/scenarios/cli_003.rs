use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_contains, expect_success};

#[e2e(
    story = "PARK-CLI-003",
    scope = "aliases",
    priority = "P1",
    description = "Launch a command with the explicit run alias",
    tags = ["cli", "smoke", "aliases"]
)]
pub fn use_explicit_run_alias() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-CLI-003")?;

    let launch = environment.run(&["run", "worker", "--", "/bin/true"])?;
    expect_success("run alias", &launch)?;

    let wait = environment.run(&["wait", "worker", "--exit"])?;
    expect_success("wait", &wait)?;

    let status = environment.run(&["status", "worker", "--json"])?;
    expect_success("status", &status)?;
    expect_contains(
        &String::from_utf8_lossy(&status.stdout),
        r#""state":"exited""#,
    )?;
    Ok(())
}
