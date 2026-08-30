use std::env;
use std::fs;
use std::process::Command;

use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_contains, expect_exit, expect_stderr_nonempty};

#[e2e(
    story = "PARK-SCOPE-005",
    scope = "project-resolution",
    priority = "P1",
    description = "Report a deleted current project without starting Park state",
    tags = ["scope", "errors", "paths"]
)]
pub fn reject_deleted_current_project() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-SCOPE-005")?;
    let invalid_project = environment.root_path().join("deleted-project");
    fs::create_dir(&invalid_project)
        .map_err(|error| format!("create invalid project directory: {error}"))?;
    let park = env::var_os("PARK_BIN").ok_or_else(|| "PARK_BIN is not set".to_owned())?;
    let output = Command::new("/bin/sh")
        .arg("-c")
        .arg("rmdir \"$1\" || exit 99; exec \"$2\" ps")
        .arg("sh")
        .arg(&invalid_project)
        .arg(&park)
        .current_dir(&invalid_project)
        .env("HOME", environment.root_path().join("home"))
        .env("XDG_STATE_HOME", environment.root_path().join("state"))
        .env("XDG_RUNTIME_DIR", environment.root_path().join("runtime"))
        .env("PARK_E2E_SCENARIO", "PARK-SCOPE-005")
        .output()
        .map_err(|error| format!("execute park from deleted project: {error}"))?;
    expect_exit("invalid project", &output, 1)?;
    expect_stderr_nonempty("invalid project", &output)?;
    expect_contains(
        &String::from_utf8_lossy(&output.stderr),
        "current directory",
    )?;

    if environment.root_path().join("state").exists()
        || environment.root_path().join("runtime").exists()
    {
        return Err("invalid project request created Park state".to_owned());
    }
    Ok(())
}
