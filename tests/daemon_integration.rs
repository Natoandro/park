use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use serde_json::Value;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct TestEnvironment {
    root: PathBuf,
}

impl TestEnvironment {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "park-phase4-integration-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).expect("test root should be created");
        Self { root }
    }

    fn runtime_dir(&self) -> PathBuf {
        self.root.join("runtime")
    }

    fn run(&self, args: &[&str]) -> Output {
        run_with_root(&self.root, args)
    }

    fn run_from(&self, directory: &Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_park"))
            .args(args)
            .current_dir(directory)
            .env("XDG_STATE_HOME", self.root.join("state"))
            .env("XDG_RUNTIME_DIR", self.root.join("runtime"))
            .env("XDG_CONFIG_HOME", self.root.join("config"))
            .output()
            .expect("park command should run")
    }

    fn run_with_vars(&self, args: &[&str], vars: &[(&str, &str)]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_park"));
        command
            .args(args)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .env("XDG_STATE_HOME", self.root.join("state"))
            .env("XDG_RUNTIME_DIR", self.root.join("runtime"))
            .env("XDG_CONFIG_HOME", self.root.join("config"))
            .envs(vars.iter().copied());
        command.output().expect("park command should run")
    }

    fn pid_path(&self) -> PathBuf {
        self.runtime_dir().join("park/daemon.pid")
    }

    fn socket_path(&self) -> PathBuf {
        self.runtime_dir().join("park/daemon.sock")
    }

    fn write_config(&self, contents: &str) {
        let path = self.root.join("config/park/config.toml");
        fs::create_dir_all(path.parent().expect("config parent should exist"))
            .expect("config directory should be created");
        fs::write(path, contents).expect("config should be written");
    }

    fn daemon_pid(&self) -> Option<i32> {
        fs::read_to_string(self.pid_path())
            .ok()
            .and_then(|value| value.trim().parse().ok())
    }
}

fn run_with_root(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_park"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_RUNTIME_DIR", root.join("runtime"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .output()
        .expect("park command should run")
}

#[test]
fn reports_daemon_status_as_structured_data() {
    let environment = TestEnvironment::new();
    let output = environment.run(&["daemon", "status", "--json"]);
    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let result: Value = serde_json::from_slice(&output.stdout).expect("result should be JSON");
    assert_eq!(result["status"], "success");
    assert!(result["data"]["pid"].as_u64().is_some());
    assert_eq!(result["data"]["binary_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(result["data"]["protocol_version"], 1);
    assert_eq!(result["data"]["handoff_version"], 0);
    assert_eq!(result["data"]["generation"], 1);
    assert_eq!(result["data"]["reexec_state"], "serving");
    assert_eq!(result["data"]["active_record_count"], 0);
}

#[test]
fn reports_effective_daemon_configuration_and_source() {
    let environment = TestEnvironment::new();
    let output = environment.run(&["daemon", "config", "--json"]);
    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let result: Value = serde_json::from_slice(&output.stdout).expect("result should be JSON");
    assert_eq!(result["status"], "success");
    assert_eq!(result["data"]["source"], "defaults");
    assert!(result["data"]["path"].as_str().is_some());
    assert_eq!(
        result["data"]["config"]["daemon"]["reexec"]["active_processes"],
        "defer"
    );
    assert_eq!(
        result["data"]["config"]["managed_processes"]["restart"]["policy"],
        "never"
    );
}

#[test]
fn reports_values_from_the_user_config_file() {
    let environment = TestEnvironment::new();
    environment.write_config(
        "[daemon.reexec]\nactive_processes = \"restart\"\n[managed_processes.restart]\npolicy = \"on-failure\"\n",
    );
    let output = environment.run(&["daemon", "config", "--json"]);
    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let result: Value = serde_json::from_slice(&output.stdout).expect("result should be JSON");
    assert_eq!(result["data"]["source"], "file");
    assert_eq!(
        result["data"]["config"]["daemon"]["reexec"]["active_processes"],
        "restart"
    );
    assert_eq!(
        result["data"]["config"]["managed_processes"]["restart"]["policy"],
        "on-failure"
    );
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        if let Some(pid) = self.daemon_pid()
            && pid > 1
            && pid != std::process::id() as i32
        {
            let _ = kill(Pid::from_raw(pid), Signal::SIGTERM);
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn starts_on_demand_and_reconnects_to_the_same_daemon() {
    let environment = TestEnvironment::new();
    let first = environment.run(&["ps", "--json"]);
    assert!(first.status.success(), "stderr: {:?}", first.stderr);
    assert_eq!(
        String::from_utf8_lossy(&first.stdout).trim(),
        r#"{"status":"success","ok":true,"data":[]}"#
    );

    let second = environment.run(&["ps", "--json"]);
    assert!(second.status.success(), "stderr: {:?}", second.stderr);
    assert_eq!(
        String::from_utf8_lossy(&second.stdout).trim(),
        r#"{"status":"success","ok":true,"data":[]}"#
    );
}

#[test]
fn lists_records_with_explicit_project_scopes() {
    let environment = TestEnvironment::new();
    let project = environment.root.join("project");
    let nested = project.join("nested");
    let sibling = environment.root.join("project-sibling");
    fs::create_dir_all(&nested).expect("nested project should be created");
    fs::create_dir(&sibling).expect("sibling project should be created");

    for (directory, name) in [
        (&project, "root"),
        (&nested, "nested"),
        (&sibling, "sibling"),
    ] {
        let launch = environment.run_from(directory, &[name, "--", "/bin/true"]);
        assert!(launch.status.success(), "stderr: {:?}", launch.stderr);
    }

    let current = environment.run_from(&project, &["ps", "--json"]);
    assert!(current.status.success(), "stderr: {:?}", current.stderr);
    let current: Value =
        serde_json::from_slice(&current.stdout).expect("current ps should be JSON");
    let current_records = current["data"]
        .as_array()
        .expect("current data should be an array");
    assert_eq!(current_records.len(), 1);
    assert_eq!(current_records[0]["key"]["name"], "root");
    assert_eq!(
        current_records[0]["key"]["project_path"],
        project.to_string_lossy().as_ref()
    );

    let subtree = environment.run_from(&project, &["ps", "--scope", "subtree", "--json"]);
    assert!(subtree.status.success(), "stderr: {:?}", subtree.stderr);
    let subtree: Value =
        serde_json::from_slice(&subtree.stdout).expect("subtree ps should be JSON");
    assert_eq!(subtree["data"]["scope"], "subtree");
    let subtree_records = subtree["data"]["records"]
        .as_array()
        .expect("subtree records should be an array");
    assert_eq!(subtree_records.len(), 2);
    assert!(subtree_records.iter().all(|record| {
        record["key"]["project_path"] == project.to_string_lossy().as_ref()
            || record["key"]["project_path"] == nested.to_string_lossy().as_ref()
    }));
    assert!(
        !subtree_records
            .iter()
            .any(|record| record["key"]["project_path"] == sibling.to_string_lossy().as_ref())
    );

    let global = environment.run_from(&project, &["ps", "--scope", "global", "--json"]);
    assert!(global.status.success(), "stderr: {:?}", global.stderr);
    let global: Value = serde_json::from_slice(&global.stdout).expect("global ps should be JSON");
    assert_eq!(global["data"]["scope"], "global");
    let global_records = global["data"]["records"]
        .as_array()
        .expect("global records should be an array");
    assert_eq!(global_records.len(), 3);
    let projects = global_records
        .iter()
        .map(|record| {
            record["key"]["project_path"]
                .as_str()
                .expect("project path")
        })
        .collect::<Vec<_>>();
    assert!(projects.windows(2).all(|pair| pair[0] <= pair[1]));

    let human = environment.run_from(&project, &["ps", "--scope", "subtree"]);
    assert!(human.status.success(), "stderr: {:?}", human.stderr);
    let human = String::from_utf8_lossy(&human.stdout);
    assert!(human.starts_with("PROJECT"));
    assert!(human.contains(project.to_string_lossy().as_ref()));
    assert!(human.contains(nested.to_string_lossy().as_ref()));
    assert!(!human.contains(sibling.to_string_lossy().as_ref()));
}

#[test]
fn replaces_stale_runtime_state_before_serving() {
    let environment = TestEnvironment::new();
    let first = environment.run(&["ps", "--json"]);
    assert!(first.status.success(), "stderr: {:?}", first.stderr);
    let pid = environment
        .daemon_pid()
        .expect("daemon marker should exist");
    kill(Pid::from_raw(pid), Signal::SIGTERM).expect("daemon should terminate");
    thread::sleep(Duration::from_millis(100));

    let _ = fs::remove_file(environment.socket_path());
    fs::write(environment.socket_path(), b"stale socket")
        .expect("stale socket marker should be writable");
    fs::write(environment.pid_path(), b"999\n").expect("stale pid marker should be writable");

    let replacement = environment.run(&["ps", "--json"]);
    assert!(
        replacement.status.success(),
        "stderr: {:?}",
        replacement.stderr
    );
    assert_eq!(
        String::from_utf8_lossy(&replacement.stdout).trim(),
        r#"{"status":"success","ok":true,"data":[]}"#
    );
}

#[test]
fn concurrent_first_clients_share_one_daemon_owner() {
    let environment = TestEnvironment::new();
    let root = environment.root.clone();
    let clients = (0..4)
        .map(|_| {
            let root = root.clone();
            thread::spawn(move || run_with_root(&root, &["ps", "--json"]))
        })
        .collect::<Vec<_>>();

    for client in clients {
        let output = client.join().expect("client thread should finish");
        assert!(output.status.success(), "stderr: {:?}", output.stderr);
    }
    assert!(
        environment.daemon_pid().is_some(),
        "one daemon should remain"
    );
}

#[test]
fn status_returns_the_missing_record_exit_code() {
    let environment = TestEnvironment::new();
    let output = environment.run(&["status", "dev", "--json"]);
    assert_eq!(output.status.code(), Some(3), "stderr: {:?}", output.stderr);
    assert!(String::from_utf8_lossy(&output.stdout).contains("missing_record"));
}

#[test]
fn launches_captures_both_streams_and_retains_the_terminal_record() {
    let environment = TestEnvironment::new();
    let launch = environment.run(&[
        "streams",
        "--",
        "/bin/sh",
        "-c",
        "printf stdout; printf stderr >&2",
    ]);
    assert!(launch.status.success(), "stderr: {:?}", launch.stderr);
    let status = environment.run(&["status", "streams", "--json"]);
    assert!(status.status.success(), "stderr: {:?}", status.stderr);
    let record: Value = serde_json::from_slice(&status.stdout).expect("status data should be JSON");
    let stdout_path = record["data"]["logs"]["stdout"]
        .as_str()
        .expect("stdout path should be returned");
    let stderr_path = record["data"]["logs"]["stderr"]
        .as_str()
        .expect("stderr path should be returned");
    assert_eq!(wait_for_state(&environment, "streams"), "exited");
    assert_eq!(
        fs::read_to_string(stdout_path).expect("stdout log should be readable"),
        "stdout"
    );
    assert_eq!(
        fs::read_to_string(stderr_path).expect("stderr log should be readable"),
        "stderr"
    );
}

#[test]
fn captures_client_environment_and_rereads_dotenv_files_in_the_daemon() {
    let environment = TestEnvironment::new();
    let dotenv = environment.root.join("test.env");
    fs::write(&dotenv, "PARK_DOTENV=dotenv\nPARK_CAPTURE=from-dotenv\n")
        .expect("dotenv file should be written");
    let dotenv = dotenv.to_string_lossy().into_owned();
    let launch = environment.run_with_vars(
        &[
            "env-capture",
            "--env-file",
            &dotenv,
            "--",
            "/bin/sh",
            "-c",
            "printf '%s:%s' \"$PARK_DOTENV\" \"$PARK_CAPTURE\"",
        ],
        &[("PARK_CAPTURE", "captured")],
    );
    assert!(launch.status.success(), "stderr: {:?}", launch.stderr);
    assert_eq!(wait_for_state(&environment, "env-capture"), "exited");
    let logs = environment.run(&["logs", "env-capture", "--stdout"]);
    assert!(logs.status.success(), "stderr: {:?}", logs.stderr);
    assert_eq!(String::from_utf8_lossy(&logs.stdout), "dotenv:captured");

    fs::write(&dotenv, "PARK_DOTENV=changed\nPARK_CAPTURE=changed\n")
        .expect("dotenv file should be updated");
    let restart = environment.run(&["restart", "env-capture"]);
    assert!(restart.status.success(), "stderr: {:?}", restart.stderr);
    assert_eq!(wait_for_state(&environment, "env-capture"), "exited");
    let logs = environment.run(&["logs", "env-capture", "--stdout"]);
    assert!(logs.status.success(), "stderr: {:?}", logs.stderr);
    assert_eq!(
        String::from_utf8_lossy(&logs.stdout),
        "dotenv:capturedchanged:captured"
    );

    let recapture = environment.run_with_vars(
        &["restart", "env-capture", "--recapture-env"],
        &[("PARK_CAPTURE", "recaptured")],
    );
    assert!(recapture.status.success(), "stderr: {:?}", recapture.stderr);
    assert_eq!(wait_for_state(&environment, "env-capture"), "exited");
    let logs = environment.run(&["logs", "env-capture", "--stdout"]);
    assert!(logs.status.success(), "stderr: {:?}", logs.stderr);
    assert_eq!(
        String::from_utf8_lossy(&logs.stdout),
        "dotenv:capturedchanged:capturedchanged:recaptured"
    );
}

#[test]
fn inspects_and_updates_explicit_environment_values() {
    let environment = TestEnvironment::new();
    let launch = environment.run(&["env-update", "--", "/bin/sleep", "30"]);
    assert!(launch.status.success(), "stderr: {:?}", launch.stderr);

    let update = environment.run(&[
        "env",
        "env-update",
        "--set",
        "PARK_EXPLICIT=one=two",
        "--unset",
        "PARK_CAPTURED",
        "--json",
    ]);
    assert!(update.status.success(), "stderr: {:?}", update.stderr);
    let response: Value = serde_json::from_slice(&update.stdout).expect("env should be JSON");
    let status = environment.run(&["status", "env-update", "--json"]);
    assert!(status.status.success(), "stderr: {:?}", status.stderr);
    let status: Value = serde_json::from_slice(&status.stdout).expect("status should be JSON");
    assert!(status["data"].get("environment").is_none());
    let variables = response["data"]["variables"]
        .as_array()
        .expect("variables should be an array");
    assert!(
        variables.iter().any(|variable| {
            variable["key"] == "PARK_EXPLICIT" && variable["value"] == "one=two"
        })
    );
    assert!(
        !variables
            .iter()
            .any(|variable| variable["key"] == "PARK_CAPTURED")
    );

    let stop = environment.run(&["stop", "env-update", "--force"]);
    assert!(stop.status.success(), "stderr: {:?}", stop.stderr);
}

#[test]
fn start_with_a_command_creates_a_new_record() {
    let environment = TestEnvironment::new();
    let start = environment.run(&["start", "created", "--", "/bin/true"]);
    assert!(start.status.success(), "stderr: {:?}", start.stderr);
    assert_eq!(wait_for_state(&environment, "created"), "exited");

    let duplicate = environment.run(&["start", "created", "--", "/bin/true"]);
    assert_eq!(
        duplicate.status.code(),
        Some(4),
        "stderr: {:?}",
        duplicate.stderr
    );
}

#[test]
fn rejects_duplicate_keys_and_retains_spawn_failures() {
    let environment = TestEnvironment::new();
    let first = environment.run(&["duplicate", "--", "/bin/true"]);
    assert!(first.status.success(), "stderr: {:?}", first.stderr);
    let duplicate = environment.run(&["duplicate", "--", "/bin/true"]);
    assert_eq!(
        duplicate.status.code(),
        Some(4),
        "stderr: {:?}",
        duplicate.stderr
    );

    let failed = environment.run(&["failed", "--", "/park/command/does-not-exist"]);
    assert_eq!(failed.status.code(), Some(1), "stderr: {:?}", failed.stderr);
    assert_eq!(wait_for_state(&environment, "failed"), "failed");
}

#[test]
fn concurrent_same_key_launches_return_one_success_and_duplicate_errors() {
    let environment = TestEnvironment::new();
    let root = environment.root.clone();
    let clients = (0..4)
        .map(|_| {
            let root = root.clone();
            thread::spawn(move || {
                run_with_root(&root, &["contended", "--", "/bin/sh", "-c", "sleep 1"])
            })
        })
        .collect::<Vec<_>>();
    let outputs = clients
        .into_iter()
        .map(|client| client.join().expect("client thread should finish"))
        .collect::<Vec<_>>();

    assert_eq!(
        outputs
            .iter()
            .filter(|output| output.status.success())
            .count(),
        1
    );
    assert_eq!(
        outputs
            .iter()
            .filter(|output| output.status.code() == Some(4))
            .count(),
        3
    );
}

#[test]
fn stale_logs_without_a_record_do_not_block_a_later_launch() {
    let environment = TestEnvironment::new();
    let first = environment.run(&["stale", "--", "/bin/true"]);
    assert!(first.status.success(), "stderr: {:?}", first.stderr);
    assert_eq!(wait_for_state(&environment, "stale"), "exited");
    let database = environment.root.join("state/park/park.sqlite3");
    let connection = rusqlite::Connection::open(&database).expect("database should open");
    connection
        .execute("DELETE FROM process_records", [])
        .expect("record should be removable for this test");

    let second = environment.run(&["stale", "--", "/bin/true"]);
    assert!(second.status.success(), "stderr: {:?}", second.stderr);
}

#[test]
fn drains_large_output_without_blocking_the_child() {
    let environment = TestEnvironment::new();
    let launch = environment.run(&[
        "large-output",
        "--",
        "/bin/sh",
        "-c",
        "head -c 200000 /dev/zero",
    ]);
    assert!(launch.status.success(), "stderr: {:?}", launch.stderr);
    let status = environment.run(&["status", "large-output", "--json"]);
    assert!(status.status.success(), "stderr: {:?}", status.stderr);
    let record: Value = serde_json::from_slice(&status.stdout).expect("status data should be JSON");
    let stdout_path = record["data"]["logs"]["stdout"]
        .as_str()
        .expect("stdout path should be returned");
    assert_eq!(wait_for_state(&environment, "large-output"), "exited");
    assert_eq!(
        fs::metadata(stdout_path)
            .expect("stdout log should exist")
            .len(),
        200_000
    );
    let logs = environment.run(&["logs", "large-output"]);
    assert!(logs.status.success(), "stderr: {:?}", logs.stderr);
    assert_eq!(logs.stdout.len(), 200_000);
}

#[test]
fn reads_retained_logs_with_stream_selection_and_filters() {
    let environment = TestEnvironment::new();
    let launch = environment.run(&[
        "inspect",
        "--",
        "/bin/sh",
        "-c",
        "printf 'one\\nkeep\\nlast\\n'; printf 'err\\nkeep err\\n' >&2",
    ]);
    assert!(launch.status.success(), "stderr: {:?}", launch.stderr);
    assert_eq!(wait_for_state(&environment, "inspect"), "exited");

    let stdout = environment.run(&["logs", "inspect", "--stdout"]);
    assert!(stdout.status.success(), "stderr: {:?}", stdout.stderr);
    assert_eq!(String::from_utf8_lossy(&stdout.stdout), "one\nkeep\nlast\n");

    let stderr = environment.run(&["logs", "inspect", "--stderr"]);
    assert!(stderr.status.success(), "stderr: {:?}", stderr.stderr);
    assert_eq!(String::from_utf8_lossy(&stderr.stdout), "err\nkeep err\n");

    let filtered = environment.run(&["logs", "inspect", "--grep", "keep", "--tail", "1"]);
    assert!(filtered.status.success(), "stderr: {:?}", filtered.stderr);
    assert_eq!(String::from_utf8_lossy(&filtered.stdout), "keep err\n");

    let json = environment.run(&["logs", "inspect", "--stdout", "--json"]);
    assert!(json.status.success(), "stderr: {:?}", json.stderr);
    let response: Value = serde_json::from_slice(&json.stdout).expect("logs should be JSON");
    assert_eq!(response["data"]["stream"], "stdout");
    assert_eq!(response["data"]["content"], "one\nkeep\nlast\n");
    assert_eq!(response["data"]["state"], "exited");
}

#[test]
fn follows_log_output_until_the_process_exits() {
    let environment = TestEnvironment::new();
    let launch = environment.run(&[
        "follow",
        "--",
        "/bin/sh",
        "-c",
        "printf first; sleep .2; printf second",
    ]);
    assert!(launch.status.success(), "stderr: {:?}", launch.stderr);

    let output = environment.run(&["logs", "follow", "--follow"]);
    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "firstsecond");
}

#[test]
fn reads_empty_retained_logs_successfully() {
    let environment = TestEnvironment::new();
    let launch = environment.run(&["empty", "--", "/bin/true"]);
    assert!(launch.status.success(), "stderr: {:?}", launch.stderr);
    assert_eq!(wait_for_state(&environment, "empty"), "exited");

    let logs = environment.run(&["logs", "empty"]);
    assert!(logs.status.success(), "stderr: {:?}", logs.stderr);
    assert!(logs.stdout.is_empty());
}

#[test]
fn waits_for_state_exit_and_literal_output_matches() {
    let environment = TestEnvironment::new();
    let running = environment.run(&["waiting", "--", "/bin/sh", "-c", "sleep 1"]);
    assert!(running.status.success(), "stderr: {:?}", running.stderr);

    let running_wait = environment.run(&["wait", "waiting", "--state", "running"]);
    assert!(
        running_wait.status.success(),
        "stderr: {:?}",
        running_wait.stderr
    );
    assert!(String::from_utf8_lossy(&running_wait.stdout).contains("State: running\n"));

    let matched = environment.run(&[
        "matching",
        "--",
        "/bin/sh",
        "-c",
        "sleep .1; printf ready >&2; exit 7",
    ]);
    assert!(matched.status.success(), "stderr: {:?}", matched.stderr);
    let match_wait = environment.run(&["wait", "matching", "--match", "ready", "--timeout", "2s"]);
    assert!(
        match_wait.status.success(),
        "stderr: {:?}",
        match_wait.stderr
    );
    assert!(String::from_utf8_lossy(&match_wait.stdout).contains("State: exited\n"));

    let exit_wait = environment.run(&["wait", "matching", "--exit"]);
    assert!(exit_wait.status.success(), "stderr: {:?}", exit_wait.stderr);
    let exit_output = String::from_utf8_lossy(&exit_wait.stdout);
    assert!(exit_output.contains("State: exited\n"));
    assert!(exit_output.contains("Exit code: 7\n"));

    let stop = environment.run(&["stop", "waiting", "--force"]);
    assert!(stop.status.success(), "stderr: {:?}", stop.stderr);
}

#[test]
fn wait_times_out_and_reports_missing_records() {
    let environment = TestEnvironment::new();
    let launch = environment.run(&["timeout", "--", "/bin/sleep", "1"]);
    assert!(launch.status.success(), "stderr: {:?}", launch.stderr);

    let timeout = environment.run(&["wait", "timeout", "--exit", "--timeout", "1ms"]);
    assert_eq!(
        timeout.status.code(),
        Some(1),
        "stderr: {:?}",
        timeout.stderr
    );
    assert!(String::from_utf8_lossy(&timeout.stderr).contains("timed out"));

    let missing = environment.run(&["wait", "missing", "--exit", "--timeout", "1ms"]);
    assert_eq!(
        missing.status.code(),
        Some(3),
        "stderr: {:?}",
        missing.stderr
    );

    let stop = environment.run(&["stop", "timeout", "--force"]);
    assert!(stop.status.success(), "stderr: {:?}", stop.stderr);
}

#[test]
fn disconnected_wait_client_does_not_block_daemon_operations() {
    let environment = TestEnvironment::new();
    let launch = environment.run(&["disconnect", "--", "/bin/sleep", "1"]);
    assert!(launch.status.success(), "stderr: {:?}", launch.stderr);

    let mut waiter = Command::new(env!("CARGO_BIN_EXE_park"))
        .args(["wait", "disconnect", "--exit"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("XDG_STATE_HOME", environment.root.join("state"))
        .env("XDG_RUNTIME_DIR", environment.root.join("runtime"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("wait client should start");
    thread::sleep(Duration::from_millis(150));
    waiter.kill().expect("wait client should be killable");
    let _ = waiter.wait();

    let status = environment.run(&["status", "disconnect", "--json"]);
    assert!(status.status.success(), "stderr: {:?}", status.stderr);
    let stop = environment.run(&["stop", "disconnect", "--force"]);
    assert!(stop.status.success(), "stderr: {:?}", stop.stderr);
}

#[test]
fn stops_a_managed_process_group_and_escalates_when_forced() {
    let environment = TestEnvironment::new();
    let launch = environment.run(&["graceful", "--", "/bin/sh", "-c", "sleep 30"]);
    assert!(launch.status.success(), "stderr: {:?}", launch.stderr);
    let stop = environment.run(&["stop", "graceful"]);
    assert!(stop.status.success(), "stderr: {:?}", stop.stderr);
    assert!(matches!(
        wait_for_terminal_state(&environment, "graceful").as_str(),
        "exited" | "killed"
    ));
    let repeated = environment.run(&["stop", "graceful"]);
    assert_eq!(
        repeated.status.code(),
        Some(5),
        "stderr: {:?}",
        repeated.stderr
    );

    let force_launch = environment.run(&["forced", "--", "/bin/sleep", "30"]);
    assert!(
        force_launch.status.success(),
        "stderr: {:?}",
        force_launch.stderr
    );
    let force_stop = environment.run(&["stop", "forced", "--force"]);
    assert!(
        force_stop.status.success(),
        "stderr: {:?}",
        force_stop.stderr
    );
    assert_eq!(wait_for_terminal_state(&environment, "forced"), "killed");
}

#[test]
fn escalates_when_the_managed_command_ignores_sigterm() {
    let environment = TestEnvironment::new();
    let launch = environment.run(&["stubborn", "--", "/bin/sh", "-c", "trap '' TERM; sleep 30"]);
    assert!(launch.status.success(), "stderr: {:?}", launch.stderr);

    let stop = environment.run(&["stop", "stubborn"]);
    assert!(stop.status.success(), "stderr: {:?}", stop.stderr);
    assert_eq!(wait_for_terminal_state(&environment, "stubborn"), "killed");
}

#[test]
fn supports_named_signals_and_rejects_unknown_signals() {
    let environment = TestEnvironment::new();
    let launch = environment.run(&["signalled", "--", "/bin/sleep", "30"]);
    assert!(launch.status.success(), "stderr: {:?}", launch.stderr);

    let unknown = environment.run(&["signal", "signalled", "9"]);
    assert_eq!(
        unknown.status.code(),
        Some(1),
        "stderr: {:?}",
        unknown.stderr
    );
    let term = environment.run(&["signal", "signalled", "SIGTERM"]);
    assert!(term.status.success(), "stderr: {:?}", term.stderr);
    assert_eq!(wait_for_terminal_state(&environment, "signalled"), "killed");
}

#[test]
fn restarts_and_starts_retained_terminal_records() {
    let environment = TestEnvironment::new();
    let launch = environment.run(&["repeat", "--", "/bin/sh", "-c", "printf repeat"]);
    assert!(launch.status.success(), "stderr: {:?}", launch.stderr);
    assert_eq!(wait_for_terminal_state(&environment, "repeat"), "exited");

    let restart = environment.run(&["restart", "repeat"]);
    assert!(restart.status.success(), "stderr: {:?}", restart.stderr);
    assert_eq!(wait_for_terminal_state(&environment, "repeat"), "exited");

    let start = environment.run(&["start", "repeat"]);
    assert!(start.status.success(), "stderr: {:?}", start.stderr);
    assert_eq!(wait_for_terminal_state(&environment, "repeat"), "exited");

    let logs = environment.run(&["logs", "repeat", "--stdout"]);
    assert!(logs.status.success(), "stderr: {:?}", logs.stderr);
    assert_eq!(String::from_utf8_lossy(&logs.stdout), "repeatrepeatrepeat");
}

#[test]
fn restart_stops_an_active_record_before_relaunching_it() {
    let environment = TestEnvironment::new();
    let launch = environment.run(&["relaunch", "--", "/bin/sleep", "30"]);
    assert!(launch.status.success(), "stderr: {:?}", launch.stderr);

    let restart = environment.run(&["restart", "relaunch"]);
    assert!(restart.status.success(), "stderr: {:?}", restart.stderr);
    let stop = environment.run(&["stop", "relaunch", "--force"]);
    assert!(stop.status.success(), "stderr: {:?}", stop.stderr);
    assert_eq!(wait_for_terminal_state(&environment, "relaunch"), "killed");
}

#[test]
fn removes_terminal_records_and_clean_keeps_active_records() {
    let environment = TestEnvironment::new();
    let launch = environment.run(&["retained", "--", "/bin/true"]);
    assert!(launch.status.success(), "stderr: {:?}", launch.stderr);
    assert_eq!(wait_for_terminal_state(&environment, "retained"), "exited");

    let removed = environment.run(&["rm", "retained"]);
    assert!(removed.status.success(), "stderr: {:?}", removed.stderr);
    let missing = environment.run(&["status", "retained", "--json"]);
    assert_eq!(
        missing.status.code(),
        Some(3),
        "stderr: {:?}",
        missing.stderr
    );

    let active = environment.run(&["active", "--", "/bin/sleep", "30"]);
    assert!(active.status.success(), "stderr: {:?}", active.stderr);
    let remove_active = environment.run(&["rm", "active"]);
    assert_eq!(
        remove_active.status.code(),
        Some(5),
        "stderr: {:?}",
        remove_active.stderr
    );
    let clean = environment.run(&["clean"]);
    assert!(clean.status.success(), "stderr: {:?}", clean.stderr);
    let active_status = environment.run(&["status", "active", "--json"]);
    assert!(
        active_status.status.success(),
        "stderr: {:?}",
        active_status.stderr
    );
    let stop = environment.run(&["stop", "active", "--force"]);
    assert!(stop.status.success(), "stderr: {:?}", stop.stderr);
}

#[test]
fn ps_orders_records_by_name() {
    let environment = TestEnvironment::new();
    for name in ["zeta", "alpha"] {
        let launch = environment.run(&[name, "--", "/bin/true"]);
        assert!(launch.status.success(), "stderr: {:?}", launch.stderr);
        assert_eq!(wait_for_state(&environment, name), "exited");
    }

    let output = environment.run(&["ps", "--json"]);
    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    let response: Value = serde_json::from_slice(&output.stdout).expect("ps should be JSON");
    let records = response["data"]
        .as_array()
        .expect("ps data should be an array");
    assert_eq!(records[0]["key"]["name"], "alpha");
    assert_eq!(records[1]["key"]["name"], "zeta");
}

#[cfg(target_os = "linux")]
#[test]
fn daemon_crash_kills_the_managed_process_group_and_reconciles_the_record() {
    let environment = TestEnvironment::new();
    let target_pid_path = environment.root.join("target.pid");
    let command = format!("echo $$ > {}; sleep 30", target_pid_path.display());
    let launch = environment.run(&["crash", "--", "/bin/sh", "-c", &command]);
    assert!(launch.status.success(), "stderr: {:?}", launch.stderr);
    let target_pid = wait_for_pid_file(&target_pid_path);
    let daemon_pid = environment
        .daemon_pid()
        .expect("daemon marker should exist before the crash");
    kill(Pid::from_raw(daemon_pid), Signal::SIGKILL).expect("daemon should be killed");

    for _ in 0..80 {
        if kill(Pid::from_raw(target_pid), None).is_err() {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    assert!(
        kill(Pid::from_raw(target_pid), None).is_err(),
        "managed child should not survive its daemon"
    );
    assert_eq!(wait_for_state(&environment, "crash"), "exited");
}

fn wait_for_state(environment: &TestEnvironment, name: &str) -> String {
    for _ in 0..80 {
        let output = environment.run(&["status", name, "--json"]);
        if output.status.success() {
            let response: Value =
                serde_json::from_slice(&output.stdout).expect("status should be JSON");
            if let Some(state) = response["data"]["state"].as_str()
                && (state == "exited" || state == "failed")
            {
                return state.to_owned();
            }
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("process did not reach a terminal state");
}

fn wait_for_terminal_state(environment: &TestEnvironment, name: &str) -> String {
    for _ in 0..120 {
        let output = environment.run(&["status", name, "--json"]);
        if output.status.success() {
            let response: Value =
                serde_json::from_slice(&output.stdout).expect("status should be JSON");
            if let Some(state) = response["data"]["state"].as_str()
                && matches!(state, "exited" | "failed" | "killed")
            {
                return state.to_owned();
            }
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("process did not reach a terminal state");
}

fn wait_for_pid_file(path: &Path) -> i32 {
    for _ in 0..80 {
        if let Ok(value) = fs::read_to_string(path)
            && let Ok(pid) = value.trim().parse()
        {
            return pid;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("managed command did not write its PID");
}
