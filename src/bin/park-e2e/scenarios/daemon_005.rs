use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{
    TestEnvironment, expect_exit, expect_stderr_nonempty, expect_success, parse_json,
    wait_for_process_exit,
};

#[e2e(
    story = "PARK-DAEMON-005",
    scope = "filesystem-layout",
    priority = "P1",
    description = "Use the documented runtime fallback when XDG runtime is unavailable",
    tags = ["daemon", "xdg", "fallback"]
)]
pub fn fall_back_when_runtime_environment_is_unavailable() -> Result<(), String> {
    let unset = TestEnvironment::new("PARK-DAEMON-005")?;
    let unset_result = fallback_case(&unset, RuntimeSetting::Unset);
    if let Err(error) = unset_result {
        return Err(format!("unset XDG_RUNTIME_DIR case failed: {error}"));
    }

    let empty = TestEnvironment::new("PARK-DAEMON-005")?;
    let empty_result = fallback_case(&empty, RuntimeSetting::Empty);
    if let Err(error) = empty_result {
        return Err(format!("empty XDG_RUNTIME_DIR case failed: {error}"));
    }

    let unusable = TestEnvironment::new("PARK-DAEMON-005")?;
    let unusable_path = unusable.root_path().join("runtime-file");
    fs::write(&unusable_path, b"not a directory")
        .map_err(|error| format!("create unusable runtime path: {error}"))?;
    let output = run_with_environment(&unusable, RuntimeSetting::Path(&unusable_path), &["ps"])?;
    expect_exit("unusable runtime path", &output, 1)?;
    expect_stderr_nonempty("unusable runtime path", &output)
}

enum RuntimeSetting<'a> {
    Unset,
    Empty,
    Path(&'a Path),
}

fn fallback_case(
    environment: &TestEnvironment,
    runtime: RuntimeSetting<'_>,
) -> Result<(), String> {
    let result = (|| {
        let output = run_with_environment(environment, runtime, &["ps", "--json"])?;
        expect_success("fallback ps", &output)?;
        let json = parse_json("fallback ps", &output)?;
        if json.get("status").and_then(|value| value.as_str()) != Some("success") {
            return Err(format!("fallback request returned an unexpected result: {json}"));
        }
        let runtime_dir = environment.root_path().join("state/park/runtime/park");
        if !runtime_dir.join("daemon.sock").exists() {
            return Err(format!(
                "fallback daemon socket is missing: {}",
                runtime_dir.join("daemon.sock").display()
            ));
        }
        for name in ["daemon.lock", "daemon.pid"] {
            if !runtime_dir.join(name).is_file() {
                return Err(format!(
                    "fallback runtime file is missing: {}",
                    runtime_dir.join(name).display()
                ));
            }
        }
        Ok(())
    })();
    stop_daemon(&environment.root_path().join("state/park/runtime/park/daemon.pid"));
    result
}

fn run_with_environment(
    environment: &TestEnvironment,
    runtime: RuntimeSetting<'_>,
    arguments: &[&str],
) -> Result<Output, String> {
    let binary = env::var_os("PARK_BIN")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| "PARK_BIN must identify the binary under test".to_owned())?;
    if !binary.is_absolute() {
        return Err(format!("PARK_BIN must be absolute: {}", binary.display()));
    }
    let mut command = Command::new(binary);
    command
        .args(arguments)
        .current_dir(environment.project_path())
        .env("HOME", environment.root_path().join("home"))
        .env("XDG_STATE_HOME", environment.root_path().join("state"))
        .env("PARK_E2E_SCENARIO", "PARK-DAEMON-005");
    match runtime {
        RuntimeSetting::Unset => {
            command.env_remove("XDG_RUNTIME_DIR");
        }
        RuntimeSetting::Empty => {
            command.env("XDG_RUNTIME_DIR", "");
        }
        RuntimeSetting::Path(path) => {
            command.env("XDG_RUNTIME_DIR", path);
        }
    }
    command
        .output()
        .map_err(|error| format!("execute Park with custom runtime environment: {error}"))
}

fn stop_daemon(pid_path: &Path) {
    let Some(pid) = fs::read_to_string(pid_path)
        .ok()
        .and_then(|value| value.trim().parse::<i32>().ok())
        .filter(|pid| *pid > 1)
    else {
        return;
    };
    let _ = kill(Pid::from_raw(pid), Signal::SIGTERM);
    if !wait_for_process_exit(pid) {
        let _ = kill(Pid::from_raw(pid), Signal::SIGKILL);
        let _ = wait_for_process_exit(pid);
    }
}
