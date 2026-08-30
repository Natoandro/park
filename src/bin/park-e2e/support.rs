use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

pub struct TestEnvironment {
    root: PathBuf,
    project: PathBuf,
    park: PathBuf,
    story: &'static str,
}

impl TestEnvironment {
    pub fn new(story: &'static str) -> Result<Self, String> {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let parent = env::var_os("PARK_E2E_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(env::temp_dir);
        if !parent.is_absolute() {
            return Err(format!(
                "PARK_E2E_ROOT must be absolute: {}",
                parent.display()
            ));
        }
        let park = env::var_os("PARK_BIN")
            .map(PathBuf::from)
            .ok_or_else(|| "PARK_BIN must identify the binary under test".to_owned())?;
        if !park.is_absolute() {
            return Err(format!("PARK_BIN must be absolute: {}", park.display()));
        }
        fs::create_dir_all(&parent).map_err(|error| format!("create e2e root: {error}"))?;
        let root = parent.join(format!("park-e2e-{story}-{}-{id}", std::process::id()));
        fs::create_dir(&root).map_err(|error| format!("create scenario root: {error}"))?;
        set_private_permissions(&root)?;
        let project = root.join("project");
        if let Err(error) = fs::create_dir(&project) {
            let _ = fs::remove_dir_all(&root);
            return Err(format!("create project: {error}"));
        }
        let project = match fs::canonicalize(&project) {
            Ok(project) => project,
            Err(error) => {
                let _ = fs::remove_dir_all(&root);
                return Err(format!("canonicalize project: {error}"));
            }
        };
        Ok(Self {
            root,
            project,
            park,
            story,
        })
    }

    pub fn run(&self, arguments: &[&str]) -> Result<Output, String> {
        Command::new(&self.park)
            .args(arguments)
            .current_dir(&self.project)
            .env("HOME", self.root.join("home"))
            .env("XDG_STATE_HOME", self.root.join("state"))
            .env("XDG_RUNTIME_DIR", self.root.join("runtime"))
            .env("PARK_E2E_SCENARIO", self.story)
            .output()
            .map_err(|error| format!("execute {}: {error}", self.park.display()))
    }

    pub fn project_path(&self) -> &Path {
        &self.project
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        let pid_path = self.root.join("runtime/park/daemon.pid");
        let pid = fs::read_to_string(&pid_path)
            .ok()
            .and_then(|value| value.trim().parse::<i32>().ok())
            .filter(|pid| *pid > 1)
            .map(Pid::from_raw);
        if let Some(pid) = pid {
            let _ = kill(pid, Signal::SIGTERM);
            if !wait_for_process_exit(pid) {
                let _ = kill(pid, Signal::SIGKILL);
                let _ = wait_for_process_exit(pid);
            }
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn wait_for_process_exit(pid: Pid) -> bool {
    for _ in 0..100 {
        if kill(pid, None).is_err() {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    false
}

fn set_private_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("set private permissions on {}: {error}", path.display()))?;
    }
    Ok(())
}

pub fn expect_success(operation: &str, output: &Output) -> Result<(), String> {
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{operation} exited with {:?}; stdout: {}; stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

pub fn expect_contains(value: &str, expected: &str) -> Result<(), String> {
    if value.contains(expected) {
        return Ok(());
    }
    Err(format!("expected {expected:?} in {value:?}"))
}
