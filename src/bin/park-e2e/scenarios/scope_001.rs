use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-SCOPE-001",
    scope = "project-identity",
    priority = "P0",
    description = "Scope identical process names to their invocation projects",
    tags = ["scope", "projects", "smoke"]
)]
pub fn scope_names_to_invocation_project() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-SCOPE-001")?;
    let first = environment.root_path().join("first-project");
    let second = environment.root_path().join("second-project");
    fs::create_dir(&first).map_err(|error| format!("create first project: {error}"))?;
    fs::create_dir(&second).map_err(|error| format!("create second project: {error}"))?;

    for (project, marker) in [(&first, "first"), (&second, "second")] {
        let command = format!("printf {marker}");
        let launch = run_in(
            &environment,
            project,
            &["dev", "--", "/bin/sh", "-c", command.as_str()],
        )?;
        expect_success("launch", &launch)?;
        let wait = run_in(&environment, project, &["wait", "dev", "--exit"])?;
        expect_success("wait", &wait)?;
    }

    let first_ps = run_in(&environment, &first, &["ps", "--json"])?;
    expect_success("first project ps", &first_ps)?;
    assert_single_project_record(
        &parse_json("first project ps", &first_ps)?,
        &fs::canonicalize(&first).map_err(|error| format!("canonicalize first project: {error}"))?,
    )?;

    let second_ps = run_in(&environment, &second, &["ps", "--json"])?;
    expect_success("second project ps", &second_ps)?;
    assert_single_project_record(
        &parse_json("second project ps", &second_ps)?,
        &fs::canonicalize(&second)
            .map_err(|error| format!("canonicalize second project: {error}"))?,
    )?;

    for (project, label) in [(&first, "first project"), (&second, "second project")] {
        let status = run_in(&environment, project, &["status", "dev", "--json"])?;
        expect_success(label, &status)?;
        let json = parse_json(label, &status)?;
        let actual = json
            .get("data")
            .and_then(|data| data.get("key"))
            .and_then(|key| key.get("project_path"))
            .and_then(|path| path.as_str())
            .ok_or_else(|| format!("{label} status has no canonical project path"))?;
        let expected = fs::canonicalize(project)
            .map_err(|error| format!("canonicalize project for {label}: {error}"))?;
        let expected = expected.to_string_lossy().into_owned();
        if actual != expected {
            return Err(format!("{label} status used project {actual:?}, expected {expected:?}"));
        }
    }
    Ok(())
}

fn assert_single_project_record(
    json: &serde_json::Value,
    expected_project: &Path,
) -> Result<(), String> {
    let records = json
        .get("data")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("ps response has no record array: {json}"))?;
    if records.len() != 1 {
        return Err(format!("expected one project record, got {records:?}"));
    }
    let actual = records[0]
        .get("key")
        .and_then(|key| key.get("project_path"))
        .and_then(|path| path.as_str())
        .ok_or_else(|| "ps record has no project path".to_owned())?;
    if actual != expected_project.to_string_lossy() {
        return Err(format!("ps used project {actual:?}, expected {expected_project:?}"));
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
        .env("PARK_E2E_SCENARIO", "PARK-SCOPE-001")
        .output()
        .map_err(|error| format!("execute park in {}: {error}", project.display()))
}
