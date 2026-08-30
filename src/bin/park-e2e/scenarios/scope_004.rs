use std::env;
use std::path::Path;
use std::process::{Command, Output};

use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_exit, expect_stderr_nonempty, expect_success, parse_json};

#[e2e(
    story = "PARK-SCOPE-004",
    scope = "project-resolution",
    priority = "P1",
    description = "Resolve symlinked project aliases to the real project",
    tags = ["scope", "paths", "symlinks"]
)]
pub fn canonicalize_symlink_project_alias() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-SCOPE-004")?;
    let alias = environment.root_path().join("project-alias");
    #[cfg(unix)]
    std::os::unix::fs::symlink(environment.project_path(), &alias)
        .map_err(|error| format!("create project symlink: {error}"))?;
    #[cfg(not(unix))]
    return Err("symlink project aliases require Unix".to_owned());

    let launch = environment.run(&["dev", "--", "/bin/true"])?;
    expect_success("launch from real project", &launch)?;
    expect_success(
        "wait from real project",
        &environment.run(&["wait", "dev", "--exit"] )?,
    )?;

    let status = run_in(&environment, &alias, &["status", "dev", "--json"])?;
    expect_success("status from symlink project", &status)?;
    let json = parse_json("status from symlink project", &status)?;
    let project_path = json
        .get("data")
        .and_then(|data| data.get("key"))
        .and_then(|key| key.get("project_path"))
        .and_then(|path| path.as_str())
        .ok_or_else(|| "symlink status has no project path".to_owned())?;
    let expected_project = environment.project_path().to_string_lossy().into_owned();
    if project_path != expected_project {
        return Err(format!("symlink status used project {project_path:?}"));
    }

    let duplicate = run_in(&environment, &alias, &["dev", "--", "/bin/false"])?;
    expect_exit("launch through symlink alias", &duplicate, 4)?;
    expect_stderr_nonempty("launch through symlink alias", &duplicate)?;
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
        .env("PARK_E2E_SCENARIO", "PARK-SCOPE-004")
        .output()
        .map_err(|error| format!("execute park in {}: {error}", project.display()))
}
