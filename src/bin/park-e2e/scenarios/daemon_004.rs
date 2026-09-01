use std::fs;
use std::path::{Path, PathBuf};

use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_success, parse_json};

#[e2e(
    story = "PARK-DAEMON-004",
    scope = "filesystem-layout",
    priority = "P0",
    description = "Keep durable and runtime daemon state in XDG locations",
    tags = ["daemon", "xdg", "filesystem"]
)]
pub fn use_xdg_state_and_runtime_locations() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-DAEMON-004")?;
    let launch = environment.run(&[
        "xdg-layout",
        "--",
        "/bin/sh",
        "-c",
        "printf state-marker",
    ])?;
    expect_success("launch", &launch)?;

    let status = environment.run(&["status", "xdg-layout", "--json"])?;
    expect_success("status", &status)?;
    let status_json = parse_json("status", &status)?;
    let status_record = status_json
        .get("data")
        .ok_or_else(|| "status response is missing its record".to_owned())?;
    let stdout_log = path_from_record(status_record, "stdout")?;
    let stderr_log = path_from_record(status_record, "stderr")?;
    let state_dir = environment.root_path().join("state/park");
    let runtime_dir = environment.root_path().join("runtime/park");

    require_directory("state directory", &state_dir)?;
    require_directory("logs directory", &state_dir.join("logs"))?;
    require_file("SQLite database", &state_dir.join("park.sqlite3"))?;
    require_file_under("stdout log", &stdout_log, &state_dir.join("logs"))?;
    require_file_under("stderr log", &stderr_log, &state_dir.join("logs"))?;
    require_directory("runtime directory", &runtime_dir)?;
    require_path("daemon socket", &runtime_dir.join("daemon.sock"))?;
    require_file("daemon lock", &runtime_dir.join("daemon.lock"))?;
    require_file("daemon PID marker", &runtime_dir.join("daemon.pid"))?;

    let wait = environment.run(&["wait", "xdg-layout", "--exit"])?;
    expect_success("wait", &wait)?;
    if !stdout_log.starts_with(state_dir.join("logs"))
        || !stderr_log.starts_with(state_dir.join("logs"))
    {
        return Err("launch returned a log outside the configured state directory".to_owned());
    }
    Ok(())
}

fn path_from_record(record: &serde_json::Value, stream: &str) -> Result<PathBuf, String> {
    record
        .get("logs")
        .and_then(|logs| logs.get(stream))
        .and_then(|path| path.as_str())
        .map(PathBuf::from)
        .ok_or_else(|| format!("launch record is missing the {stream} log path: {record}"))
}

fn require_directory(label: &str, path: &Path) -> Result<(), String> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(format!("{label} is not a directory: {}", path.display())),
        Err(error) => Err(format!("{label} is unavailable at {}: {error}", path.display())),
    }
}

fn require_file(label: &str, path: &Path) -> Result<(), String> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(format!("{label} is not a regular file: {}", path.display())),
        Err(error) => Err(format!("{label} is unavailable at {}: {error}", path.display())),
    }
}

fn require_path(label: &str, path: &Path) -> Result<(), String> {
    if path.exists() {
        Ok(())
    } else {
        Err(format!("{label} is unavailable at {}", path.display()))
    }
}

fn require_file_under(label: &str, path: &Path, directory: &Path) -> Result<(), String> {
    if !path.starts_with(directory) {
        return Err(format!(
            "{label} is outside {}: {}",
            directory.display(),
            path.display()
        ));
    }
    require_file(label, path)
}
