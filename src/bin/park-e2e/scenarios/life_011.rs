use std::fs;
use std::path::{Path, PathBuf};

use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{TestEnvironment, expect_exit, expect_success, parse_json};

#[e2e(
    story = "PARK-LIFE-011",
    scope = "remove-retention",
    priority = "P1",
    description = "Remove metadata while retaining both log streams",
    tags = ["lifecycle", "remove", "logs"]
)]
pub fn remove_record_keep_logs() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LIFE-011")?;
    let launch = environment.run(&[
        "keep-logs",
        "--",
        "/bin/sh",
        "-c",
        "printf keep-stdout; printf keep-stderr >&2",
    ])?;
    expect_success("launch", &launch)?;
    expect_success(
        "wait",
        &environment.run(&["wait", "keep-logs", "--exit"])? ,
    )?;
    let before = log_snapshot(environment.root_path())?;
    if before.len() != 2 {
        return Err(format!("expected two log files, found {}", before.len()));
    }

    let remove = environment.run(&["rm", "keep-logs", "--keep-logs"])?;
    expect_success("remove with kept logs", &remove)?;
    let status = environment.run(&["status", "keep-logs", "--json"])?;
    expect_exit("removed status", &status, 3)?;
    let status_json = parse_json("removed status", &status)?;
    if status_json.get("status").and_then(|value| value.as_str()) != Some("missing_record") {
        return Err(format!("removed status has the wrong result: {status_json}"));
    }
    let logs = environment.run(&["logs", "keep-logs", "--json"])?;
    expect_exit("unaddressable logs", &logs, 3)?;

    let after = log_snapshot(environment.root_path())?;
    if before != after {
        return Err("--keep-logs did not preserve the log files exactly".to_owned());
    }
    let records = environment.run(&["ps", "--json"])?;
    expect_success("ps", &records)?;
    let records = parse_json("ps", &records)?;
    if records
        .get("data")
        .and_then(|data| data.as_array())
        .is_none_or(|records| !records.is_empty())
    {
        return Err(format!("kept logs were mistaken for a record: {records}"));
    }
    Ok(())
}

fn log_snapshot(root: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
    let directory = root.join("state/park/logs");
    let mut files = fs::read_dir(&directory)
        .map_err(|error| format!("read log directory {}: {error}", directory.display()))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(|kind| kind.is_file())
                .map(|_| entry.path())
        })
        .map(|path| {
            let content = fs::read(&path).map_err(|error| format!("read log {}: {error}", path.display()))?;
            Ok((path, content))
        })
        .collect::<Result<Vec<_>, String>>()?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}
