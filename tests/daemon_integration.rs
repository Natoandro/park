use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

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
