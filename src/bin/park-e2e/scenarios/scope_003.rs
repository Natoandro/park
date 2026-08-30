use std::fs;
use std::path::Path;

use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-SCOPE-003",
    scope = "project-resolution",
    priority = "P1",
    description = "Use one namespace for equivalent project path spellings",
    tags = ["scope", "paths", "canonicalization"]
)]
pub fn canonicalize_relative_project_paths() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-SCOPE-003")?;
    let nested = environment.project_path().join("nested");
    fs::create_dir(&nested).map_err(|error| format!("create nested directory: {error}"))?;

    let launch_dir = nested.join("..");
    let launch = run_in(&environment, &launch_dir, &["dev", "--", "/bin/true"])?;
    expect_success("launch from equivalent path", &launch)?;
    let wait = run_in(
        &environment,
        environment.project_path(),
        &["wait", "dev", "--exit"],
    )?;
    expect_success("wait from canonical path", &wait)?;

    let status = run_in(
        &environment,
        environment.project_path(),
        &["status", "dev", "--json"],
    )?;
    expect_success("status from canonical path", &status)?;
    let status_json = parse_json("status", &status)?;
    let record = status_json
        .get("data")
        .ok_or_else(|| "status response has no record".to_owned())?;
    let project_path = record
        .get("key")
        .and_then(|key| key.get("project_path"))
        .and_then(|path| path.as_str())
        .ok_or_else(|| "status record has no project path".to_owned())?;
    let expected = environment.project_path().to_string_lossy().into_owned();
    if project_path != expected {
        return Err(format!("project path was {project_path:?}, expected {expected:?}"));
    }

    let ps = run_in(&environment, environment.project_path(), &["ps", "--json"])?;
    expect_success("ps", &ps)?;
    let ps_json = parse_json("ps", &ps)?;
    let records = ps_json
        .get("data")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "ps response has no record array".to_owned())?;
    if records.len() != 1 {
        return Err(format!("equivalent paths created {} records", records.len()));
    }
    Ok(())
}

fn run_in(
    environment: &TestEnvironment,
    project: &Path,
    arguments: &[&str],
) -> Result<std::process::Output, String> {
    let park = std::env::var_os("PARK_BIN").ok_or_else(|| "PARK_BIN is not set".to_owned())?;
    std::process::Command::new(park)
        .args(arguments)
        .current_dir(project)
        .env("HOME", environment.root_path().join("home"))
        .env("XDG_STATE_HOME", environment.root_path().join("state"))
        .env("XDG_RUNTIME_DIR", environment.root_path().join("runtime"))
        .env("PARK_E2E_SCENARIO", "PARK-SCOPE-003")
        .output()
        .map_err(|error| format!("execute park in {}: {error}", project.display()))
}
