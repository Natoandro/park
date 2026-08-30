use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_contains, expect_success};

#[e2e(
    story = "PARK-CLI-001",
    scope = "launch",
    priority = "P0",
    description = "Launch a named command and inspect its active record",
    tags = ["smoke", "cli", "lifecycle"]
)]
pub fn launch_named_command() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-CLI-001")?;

    let launch = environment.run(&["dev", "--", "/bin/sleep", "30"])?;
    expect_success("launch", &launch)?;

    let status = environment.run(&["status", "dev", "--json"])?;
    expect_success("status", &status)?;
    let status_text = String::from_utf8_lossy(&status.stdout);
    expect_contains(&status_text, r#""status":"success""#)?;
    expect_contains(&status_text, r#""ok":true"#)?;
    expect_contains(&status_text, r#""state":"running""#)?;
    let project = environment
        .project_path()
        .to_str()
        .ok_or_else(|| "project path is not valid UTF-8".to_owned())?;
    expect_contains(&status_text, &format!(r#""project_path":"{project}""#))?;
    expect_contains(&status_text, &format!(r#""working_directory":"{project}""#))?;

    let stop = environment.run(&["stop", "dev", "--force"])?;
    expect_success("force stop", &stop)?;

    let removed = environment.run(&["rm", "dev"])?;
    expect_success("remove", &removed)?;
    Ok(())
}
