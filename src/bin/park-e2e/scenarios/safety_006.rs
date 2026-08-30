use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_exit, expect_success};

#[e2e(
    story = "PARK-SAFETY-006",
    scope = "cleanup-safety",
    priority = "P0",
    description = "Limit rm and clean to eligible Park records",
    tags = ["safety", "cli", "cleanup", "projects"]
)]
pub fn preserve_active_records_and_unrelated_files() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-SAFETY-006")?;
    let other_project = environment.root_path().join("other-project");
    fs::create_dir(&other_project)
        .map_err(|error| format!("create other project: {error}"))?;

    expect_success(
        "active launch",
        &environment.run(&["active", "--", "/bin/sleep", "30"] )?,
    )?;
    expect_success(
        "main terminal launch",
        &environment.run(&["main-terminal", "--", "/bin/true"] )?,
    )?;
    expect_success(
        "main terminal wait",
        &environment.run(&["wait", "main-terminal", "--exit"] )?,
    )?;
    expect_success(
        "other terminal launch",
        &run_in(
            &environment,
            &other_project,
            &["other-terminal", "--", "/bin/true"],
        )?,
    )?;
    expect_success(
        "other terminal wait",
        &run_in(
            &environment,
            &other_project,
            &["wait", "other-terminal", "--exit"],
        )?,
    )?;

    let unrelated = environment.root_path().join("state/park/unrelated-file");
    fs::write(&unrelated, b"do not remove")
        .map_err(|error| format!("create unrelated state file: {error}"))?;

    expect_success(
        "targeted remove",
        &environment.run(&["rm", "main-terminal"] )?,
    )?;
    expect_missing("removed main terminal", &environment.run(&["status", "main-terminal"] )?)?;
    expect_missing("removed main terminal logs", &environment.run(&["logs", "main-terminal"] )?)?;

    expect_success(
        "clean from other project",
        &run_in(&environment, &other_project, &["clean"] )?,
    )?;
    expect_missing(
        "cleaned other terminal",
        &run_in(
            &environment,
            &other_project,
            &["status", "other-terminal"],
        )?,
    )?;
    if !unrelated.is_file() {
        return Err("clean removed an unrelated state file".to_owned());
    }

    let active_status = environment.run(&["status", "active", "--json"])?;
    expect_success("active status after clean", &active_status)?;
    if !String::from_utf8_lossy(&active_status.stdout).contains(r#""state":"running""#) {
        return Err("clean removed or changed the active record".to_owned());
    }
    expect_success("force stop", &environment.run(&["stop", "active", "--force"] )?)?;
    Ok(())
}

fn expect_missing(operation: &str, output: &Output) -> Result<(), String> {
    expect_exit(operation, output, 3)?;
    if !output.stdout.is_empty() || output.stderr.is_empty() {
        return Err(format!(
            "{operation} did not report a missing record on stderr"
        ));
    }
    Ok(())
}

fn run_in(
    environment: &TestEnvironment,
    project: &Path,
    arguments: &[&str],
) -> Result<Output, String> {
    let park = env::var_os("PARK_BIN").ok_or_else(|| "PARK_BIN is not set".to_owned())?;
    Command::new(park)
        .args(arguments)
        .current_dir(project)
        .env("HOME", environment.root_path().join("home"))
        .env("XDG_STATE_HOME", environment.root_path().join("state"))
        .env("XDG_RUNTIME_DIR", environment.root_path().join("runtime"))
        .env("PARK_E2E_SCENARIO", "PARK-SAFETY-006")
        .output()
        .map_err(|error| format!("execute park in {}: {error}", project.display()))
}
