use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_exit, expect_contains, expect_success};

#[e2e(
    story = "PARK-LIFE-013",
    scope = "cleanup",
    priority = "P1",
    description = "Clean terminal records across projects while retaining active records",
    tags = ["lifecycle", "clean", "system"]
)]
pub fn clean_terminal_records_globally() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LIFE-013")?;
    let other_project = environment.root_path().join("other-project");
    fs::create_dir(&other_project).map_err(|error| format!("create other project: {error}"))?;

    let first = environment.run(&["terminal-one", "--", "/bin/true"])?;
    expect_success("first terminal launch", &first)?;
    expect_success(
        "first terminal wait",
        &environment.run(&["wait", "terminal-one", "--exit"])? ,
    )?;
    let second = run_in(&environment, &other_project, &["terminal-two", "--", "/bin/true"])?;
    expect_success("second terminal launch", &second)?;
    expect_success(
        "second terminal wait",
        &run_in(&environment, &other_project, &["wait", "terminal-two", "--exit"])? ,
    )?;
    let active = environment.run(&["active-clean", "--", "/bin/sleep", "30"])?;
    expect_success("active launch", &active)?;
    expect_success(
        "active wait",
        &environment.run(&["wait", "active-clean", "--state", "running"])? ,
    )?;

    let clean = environment.run(&["clean"])?;
    expect_success("clean", &clean)?;
    expect_contains(&String::from_utf8_lossy(&clean.stdout), "\"removed\": 2")?;
    expect_exit(
        "first terminal status after clean",
        &environment.run(&["status", "terminal-one"])? ,
        3,
    )?;
    expect_exit(
        "second terminal status after clean",
        &run_in(&environment, &other_project, &["status", "terminal-two"])? ,
        3,
    )?;
    expect_success(
        "active status after clean",
        &environment.run(&["status", "active-clean", "--json"])? ,
    )?;
    expect_success(
        "cleanup active record",
        &environment.run(&["stop", "active-clean", "--force"])? ,
    )?;
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
        .env("PARK_E2E_SCENARIO", "PARK-LIFE-013")
        .output()
        .map_err(|error| format!("execute Park in {}: {error}", project.display()))
}
