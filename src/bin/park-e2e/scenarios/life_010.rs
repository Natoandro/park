use std::fs;
use std::path::{Path, PathBuf};

use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_exit, expect_success};

#[e2e(
    story = "PARK-LIFE-010",
    scope = "remove",
    priority = "P0",
    description = "Remove a terminal record, its logs, and nothing unrelated",
    tags = ["lifecycle", "remove", "logs"]
)]
pub fn remove_terminal_record_and_logs() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LIFE-010")?;
    launch_terminal(&environment, "remove-target", "remove-target")?;
    launch_terminal(&environment, "keep-target", "keep-target")?;
    let before = log_files(environment.root_path())?;
    if before.len() != 4 {
        return Err(format!("expected two log pairs before remove, found {}", before.len()));
    }

    let remove = environment.run(&["rm", "remove-target"])?;
    expect_success("remove", &remove)?;
    let status = environment.run(&["status", "remove-target"])?;
    expect_exit("removed status", &status, 3)?;
    let logs = environment.run(&["logs", "remove-target"])?;
    expect_exit("removed logs", &logs, 3)?;

    let after = log_files(environment.root_path())?;
    if after.len() != 2 || !after.iter().all(|path| before.contains(path)) {
        return Err(format!("remove deleted the wrong log files: before {before:?}, after {after:?}"));
    }
    let unrelated = environment.run(&["status", "keep-target", "--json"])?;
    expect_success("unrelated status", &unrelated)?;
    Ok(())
}

fn launch_terminal(
    environment: &TestEnvironment,
    name: &str,
    marker: &str,
) -> Result<(), String> {
    let launch = environment.run(&[
        name,
        "--",
        "/bin/sh",
        "-c",
        "printf '%s' \"$1\"; printf '%s' \"$1\" >&2",
        "sh",
        marker,
    ])?;
    expect_success("terminal launch", &launch)?;
    expect_success(
        "terminal wait",
        &environment.run(&["wait", name, "--exit"])? ,
    )
}

fn log_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let directory = root.join("state/park/logs");
    let mut files = fs::read_dir(&directory)
        .map_err(|error| format!("read log directory {}: {error}", directory.display()))?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_type().ok().filter(|kind| kind.is_file()).map(|_| entry.path()))
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}
