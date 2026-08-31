use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success};

#[e2e(
    story = "PARK-INSPECT-001",
    scope = "process-listing",
    priority = "P0",
    description = "List records for the current project with ps",
    tags = ["inspection", "ps", "listing"]
)]
pub fn list_records_with_ps() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-INSPECT-001")?;
    let empty = environment.run(&["ps"])?;
    expect_success("empty ps", &empty)?;
    if !empty.stderr.is_empty() || empty.stdout != b"No process records.\n" {
        return Err(format!(
            "empty ps returned unexpected output: stdout={:?}, stderr={:?}",
            empty.stdout, empty.stderr
        ));
    }

    let other_project = environment.root_path().join("other-project");
    fs::create_dir(&other_project)
        .map_err(|error| format!("create other project: {error}"))?;
    launch_terminal(&environment, environment.project_path(), "current-one")?;
    launch_terminal(&environment, environment.project_path(), "current-two")?;
    launch_terminal(&environment, &other_project, "other-project")?;

    let listed = environment.run(&["ps"])?;
    expect_success("ps", &listed)?;
    if !listed.stderr.is_empty() {
        return Err(format!(
            "ps wrote an unexpected diagnostic: {}",
            String::from_utf8_lossy(&listed.stderr)
        ));
    }
    let output = String::from_utf8_lossy(&listed.stdout);
    if !output.starts_with("NAME")
        || !output.contains("current-one")
        || !output.contains("current-two")
        || output.contains('{')
        || output.contains('[')
    {
        return Err(format!("ps human output was unexpected: {output:?}"));
    }
    Ok(())
}

fn launch_terminal(
    environment: &TestEnvironment,
    project: &Path,
    name: &str,
) -> Result<(), String> {
    let launch = run_in(environment, project, &[name, "--", "/bin/true"])?;
    expect_success("terminal launch", &launch)?;
    let wait = run_in(environment, project, &["wait", name, "--exit"])?;
    expect_success("terminal wait", &wait)
}

fn run_in(
    environment: &TestEnvironment,
    project: &Path,
    arguments: &[&str],
) -> Result<Output, String> {
    let park = std::env::var_os("PARK_BIN").ok_or_else(|| "PARK_BIN is not set".to_owned())?;
    Command::new(park)
        .args(arguments)
        .current_dir(project)
        .env("HOME", environment.root_path().join("home"))
        .env("XDG_STATE_HOME", environment.root_path().join("state"))
        .env("XDG_RUNTIME_DIR", environment.root_path().join("runtime"))
        .env("PARK_E2E_SCENARIO", "PARK-INSPECT-001")
        .output()
        .map_err(|error| format!("execute park in {}: {error}", project.display()))
}
