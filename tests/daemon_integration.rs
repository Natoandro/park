use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
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

    fn pid_path(&self) -> PathBuf {
        self.runtime_dir().join("park/daemon.pid")
    }

    fn socket_path(&self) -> PathBuf {
        self.runtime_dir().join("park/daemon.sock")
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
        .output()
        .expect("park command should run")
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        if let Some(pid) = self.daemon_pid() {
            if pid > 1 && pid != std::process::id() as i32 {
                let _ = kill(Pid::from_raw(pid), Signal::SIGTERM);
            }
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
    let launched: Value =
        serde_json::from_slice(&launch.stdout).expect("launch data should be JSON");
    let stdout_path = launched["logs"]["stdout"]
        .as_str()
        .expect("stdout path should be returned");
    let stderr_path = launched["logs"]["stderr"]
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
    let records_dir = environment.root.join("state/park/records");
    for entry in fs::read_dir(records_dir).expect("records directory should be readable") {
        let path = entry.expect("record entry should be readable").path();
        fs::remove_file(path).expect("retained record should be removable for this test");
    }

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
    let launched: Value =
        serde_json::from_slice(&launch.stdout).expect("launch data should be JSON");
    let stdout_path = launched["logs"]["stdout"]
        .as_str()
        .expect("stdout path should be returned");
    assert_eq!(wait_for_state(&environment, "large-output"), "exited");
    assert_eq!(
        fs::metadata(stdout_path)
            .expect("stdout log should exist")
            .len(),
        200_000
    );
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
            if let Some(state) = response["data"]["state"].as_str() {
                if state == "exited" || state == "failed" {
                    return state.to_owned();
                }
            }
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("process did not reach a terminal state");
}

fn wait_for_pid_file(path: &Path) -> i32 {
    for _ in 0..80 {
        if let Ok(value) = fs::read_to_string(path) {
            if let Ok(pid) = value.trim().parse() {
                return pid;
            }
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("managed command did not write its PID");
}
