use std::thread;
use std::time::Duration;

use nix::unistd::Pid;
use nix::sys::signal::{Signal, kill};
use park_e2e_macros::e2e;

use super::super::Scenario;
use super::super::support::{
    TestEnvironment, expect_success, process_is_alive, wait_for_file, wait_for_process_exit,
};

#[e2e(
    story = "PARK-LAUNCH-008",
    scope = "process-groups",
    priority = "P0",
    description = "Terminate descendants in the managed process group",
    tags = ["launch", "process", "linux"]
)]
pub fn preserve_managed_process_group() -> Result<(), String> {
    let environment = TestEnvironment::new("PARK-LAUNCH-008")?;
    let pid_path = environment.root_path().join("child.pid");
    let pid_path = pid_path
        .to_str()
        .ok_or_else(|| "child PID path is not valid UTF-8".to_owned())?;
    let launch = environment.run(&[
        "group",
        "--",
        "/bin/sh",
        "-c",
        "sleep 30 & child=$!; printf '%s' \"$child\" > \"$1\"; wait \"$child\"",
        "sh",
        pid_path,
    ])?;
    expect_success("launch", &launch)?;

    let child_pid = wait_for_file(environment.root_path().join("child.pid").as_path())?
        .trim()
        .parse::<i32>()
        .map_err(|error| format!("invalid child PID: {error}"))?;
    if !process_is_alive(child_pid) {
        return Err("child process was not alive before stopping the group".to_owned());
    }

    let stop = environment.run(&["stop", "group", "--force"])?;
    expect_success("force stop", &stop)?;
    if !wait_for_process_exit(child_pid) {
        terminate_process(child_pid);
        return Err("managed child process remained alive after group stop".to_owned());
    }
    Ok(())
}

fn terminate_process(pid: i32) {
    let _ = kill(Pid::from_raw(pid), Signal::SIGKILL);
    for _ in 0..20 {
        if !process_is_alive(pid) {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
}
