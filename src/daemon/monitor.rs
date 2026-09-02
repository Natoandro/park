use std::io;
use std::path::PathBuf;

use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStderr, ChildStdout};
use tokio::time::{Duration, sleep};

use crate::process::{ProcessKey, ProcessRecord};

use super::epoch_seconds;

pub(super) async fn monitor_child(
    storage: crate::storage::Storage,
    key: ProcessKey,
    mut child: Child,
    stdout: ChildStdout,
    stderr: ChildStderr,
) {
    let pid = child.id();
    let stdout_task = tokio::spawn(capture_output(stdout, storage_path(&storage, &key, true)));
    let stderr_task = tokio::spawn(capture_output(stderr, storage_path(&storage, &key, false)));
    tokio::pin!(stdout_task);
    tokio::pin!(stderr_task);

    let mut stdout_result = None;
    let mut stderr_result = None;
    let status = loop {
        tokio::select! {
            status = child.wait() => break status,
            result = &mut stdout_task, if stdout_result.is_none() => match capture_failure_reason("stdout", result) {
                Ok(()) => stdout_result = Some(Ok(())),
                Err(reason) => return fail_monitor(&storage, &key, &mut child, pid, reason).await,
            },
            result = &mut stderr_task, if stderr_result.is_none() => match capture_failure_reason("stderr", result) {
                Ok(()) => stderr_result = Some(Ok(())),
                Err(reason) => return fail_monitor(&storage, &key, &mut child, pid, reason).await,
            },
        }
    };
    if stdout_result.is_none() {
        stdout_result = Some(capture_failure_reason("stdout", stdout_task.await));
    }
    if stderr_result.is_none() {
        stderr_result = Some(capture_failure_reason("stderr", stderr_task.await));
    }
    if let Err(reason) = stdout_result.expect("stdout result should be set") {
        persist_monitor_failure(&storage, &key, pid, reason).await;
        return;
    }
    if let Err(reason) = stderr_result.expect("stderr result should be set") {
        persist_monitor_failure(&storage, &key, pid, reason).await;
        return;
    }
    match status {
        Ok(status) => persist_termination(&storage, &key, pid, status).await,
        Err(error) => {
            persist_monitor_failure(
                &storage,
                &key,
                pid,
                format!("could not wait for child: {error}"),
            )
            .await;
        }
    }
}

async fn fail_monitor(
    storage: &crate::storage::Storage,
    key: &ProcessKey,
    child: &mut Child,
    pid: Option<u32>,
    reason: String,
) {
    if let Some(pid) = pid {
        let _ = kill_process_group(pid);
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
    persist_monitor_failure(storage, key, pid, reason).await;
}

fn capture_failure_reason(
    stream: &str,
    result: Result<io::Result<()>, tokio::task::JoinError>,
) -> Result<(), String> {
    result
        .map_err(|error| format!("{stream} capture task failed: {error}"))?
        .map_err(|error| format!("could not capture {stream}: {error}"))
}

async fn persist_termination(
    storage: &crate::storage::Storage,
    key: &ProcessKey,
    pid: Option<u32>,
    status: std::process::ExitStatus,
) {
    let Ok(Some(mut record)) = storage.load_record(key) else {
        return;
    };
    if record.state().is_terminal() || record.pid() != pid {
        return;
    }
    let termination_signal = exit_signal(&status);
    let previous = record.clone();
    if let Err(error) = record.mark_terminated(epoch_seconds(), status.code(), termination_signal) {
        eprintln!("park daemon could not record child termination for {key:?}: {error}");
        return;
    }
    save_monitor_record(storage, key, &previous, &record).await;
}

async fn persist_monitor_failure(
    storage: &crate::storage::Storage,
    key: &ProcessKey,
    pid: Option<u32>,
    reason: String,
) {
    let record = match storage.load_record(key) {
        Ok(Some(record)) => record,
        Ok(None) => return,
        Err(error) => {
            eprintln!("park daemon could not load monitor record for {key:?}: {error}");
            return;
        }
    };
    if record.state().is_terminal() || record.pid() != pid {
        return;
    }
    let previous = record.clone();
    let mut record = record;
    if let Err(error) = record.mark_monitor_failed(epoch_seconds(), reason.clone()) {
        eprintln!("park daemon monitor failure for {key:?}: {reason}; {error}");
        return;
    }
    save_monitor_record(storage, key, &previous, &record).await;
}

async fn save_monitor_record(
    storage: &crate::storage::Storage,
    key: &ProcessKey,
    previous: &ProcessRecord,
    record: &ProcessRecord,
) {
    let mut retry_delay = Duration::from_millis(25);
    loop {
        match storage.save_record_if_unchanged(previous, record) {
            Ok(true) => return,
            Ok(false) => return,
            Err(error) => {
                eprintln!("park daemon could not persist monitor update for {key:?}: {error}");
            }
        }
        sleep(retry_delay).await;
        retry_delay = (retry_delay * 2).min(Duration::from_secs(1));
    }
}

fn storage_path(storage: &crate::storage::Storage, key: &ProcessKey, stdout: bool) -> PathBuf {
    let logs = storage.log_paths(key);
    if stdout { logs.stdout } else { logs.stderr }
}

async fn capture_output<R>(mut reader: R, path: PathBuf) -> io::Result<()>
where
    R: AsyncRead + Unpin,
{
    let mut file = OpenOptions::new().append(true).open(path).await?;
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            return Ok(());
        }
        file.write_all(&buffer[..count]).await?;
    }
}

#[cfg(unix)]
fn exit_signal(status: &std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn exit_signal(_status: &std::process::ExitStatus) -> Option<i32> {
    None
}

fn kill_process_group(pid: u32) -> nix::Result<()> {
    let pid = i32::try_from(pid)
        .ok()
        .filter(|pid| *pid > 0)
        .ok_or(nix::errno::Errno::EINVAL)?;
    killpg(Pid::from_raw(pid), Signal::SIGKILL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tokio::io::duplex;

    use crate::{ProcessRecord, Storage, StoragePaths, XdgEnvironment, resolve_project};

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn capture_reports_an_unwritable_log_destination() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime should build");
        runtime.block_on(async {
            let (_writer, reader) = duplex(16);
            let error = capture_output(reader, std::env::temp_dir())
                .await
                .expect_err("directory cannot be opened as a log file");
            assert!(matches!(error.kind(), io::ErrorKind::IsADirectory));
        });
    }

    #[test]
    fn monitor_failures_are_persisted_as_failed_records() {
        let root = std::env::temp_dir().join(format!(
            "park-monitor-test-{}-{}",
            std::process::id(),
            NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("test root should be created");
        let project_dir = root.join("project");
        fs::create_dir(&project_dir).expect("project should be created");
        let project = resolve_project(&project_dir).expect("project should resolve");
        let storage = Storage::new(
            StoragePaths::from_environment(&XdgEnvironment {
                config_home: None,
                state_home: Some(root.join("state")),
                runtime_dir: Some(root.join("runtime")),
                home: None,
            })
            .expect("paths should resolve"),
        );
        let key = ProcessKey::new(project.clone(), OsString::from("dev"));
        let mut record = ProcessRecord::new(
            key.clone(),
            project.into_path(),
            OsString::from("server"),
            vec![],
            1,
            storage.create_logs(&key).expect("logs should be created"),
        );
        record
            .mark_running(2, 123, Some(123), Some(123))
            .expect("record should be running");
        storage
            .create_record(&record)
            .expect("record should be created");

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime should build");
        runtime.block_on(persist_monitor_failure(
            &storage,
            &key,
            Some(123),
            "could not capture stdout: disk full".to_owned(),
        ));

        let record = storage
            .load_record(&key)
            .expect("record should load")
            .expect("record should exist");
        assert_eq!(record.state(), crate::ProcessState::Failed);
        assert_eq!(
            record.failure_reason(),
            Some("could not capture stdout: disk full")
        );
        fs::remove_dir_all(root).expect("test root should be removed");
    }
}
