use std::ffi::OsString;
use std::io;
use std::path::Path;

use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use tokio::process::{Child, Command};

use crate::environment::ResolvedEnvironment;
use crate::ipc::{IpcResponse, record_value};
use crate::process::{ProcessKey, ProcessRecord, validate_process_name};
use crate::project::ProjectPath;
use crate::result::ResultStatus;

use super::{DaemonState, INTERNAL_SUPERVISOR_ARGUMENT, epoch_seconds, monitor, process_identity};

pub(super) async fn start(
    state: &DaemonState,
    request_id: u64,
    project_path: ProjectPath,
    name: OsString,
    command: Vec<OsString>,
    environment: crate::environment::EnvironmentSpec,
) -> IpcResponse {
    if let Err(error) = validate_process_name(&name) {
        return IpcResponse::error(request_id, ResultStatus::Failure, error.to_string());
    }
    let key = ProcessKey::new(project_path.clone(), name);
    let lifecycle_lock = state.lifecycle_lock(&key);
    let _lifecycle_guard = lifecycle_lock.lock().await;
    let Some(_reservation) = state.reserve_launch(key.clone()) else {
        return duplicate_response(request_id);
    };
    match state.storage.load_record(&key) {
        Ok(Some(_)) => return duplicate_response(request_id),
        Ok(None) => {}
        Err(error) => return storage_failure(request_id, error.to_string()),
    }

    let mut command_parts = command.into_iter();
    let Some(executable) = command_parts.next() else {
        return IpcResponse::error(request_id, ResultStatus::Failure, "launch command is empty");
    };
    let arguments = command_parts.collect::<Vec<_>>();
    let logs = match create_logs_for_launch(&state.storage, &key) {
        Ok(logs) => logs,
        Err(error) => {
            if matches!(state.storage.load_record(&key), Ok(Some(_))) {
                return duplicate_response(request_id);
            }
            return storage_failure(request_id, error.to_string());
        }
    };
    let working_directory = project_path.as_path().to_path_buf();
    let record = ProcessRecord::new_with_environment(
        key.clone(),
        working_directory.clone(),
        executable.clone(),
        arguments.clone(),
        epoch_seconds(),
        logs,
        environment,
    );
    if let Err(error) = state.storage.create_record(&record) {
        if matches!(state.storage.load_record(&key), Ok(Some(_))) {
            return duplicate_response(request_id);
        }
        let _ = state.storage.remove_logs(&key);
        return storage_failure(request_id, error.to_string());
    }

    spawn_record(state, request_id, record).await
}

pub(super) async fn spawn_record(
    state: &DaemonState,
    request_id: u64,
    record: ProcessRecord,
) -> IpcResponse {
    spawn_record_with_previous(state, request_id, record, None).await
}

pub(super) async fn spawn_record_with_previous(
    state: &DaemonState,
    request_id: u64,
    mut record: ProcessRecord,
    previous_environment: Option<crate::environment::EnvironmentSpec>,
) -> IpcResponse {
    let key = record.key().clone();
    let resolved = match record.environment().resolve(record.working_directory()) {
        Ok(environment) => environment,
        Err(error) => {
            return persist_start_failure(
                &state.storage,
                request_id,
                &mut record,
                format!("could not resolve process environment: {error}"),
                previous_environment.as_ref(),
            );
        }
    };
    let mut child =
        match validate_executable(record.working_directory(), record.executable(), &resolved)
            .and_then(|_| {
                spawn_child(
                    record.working_directory(),
                    record.executable(),
                    record.arguments(),
                    &resolved,
                )
            }) {
            Ok(child) => child,
            Err(error) => {
                return persist_start_failure(
                    &state.storage,
                    request_id,
                    &mut record,
                    format!("could not spawn command: {error}"),
                    previous_environment.as_ref(),
                );
            }
        };
    let Some(pid) = child.id() else {
        let _ = child.kill().await;
        return persist_start_failure(
            &state.storage,
            request_id,
            &mut record,
            "spawned child did not provide a process ID",
            previous_environment.as_ref(),
        );
    };
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            return fail_after_spawn(
                &state.storage,
                request_id,
                &mut child,
                &mut record,
                "child stdout pipe was unavailable",
                previous_environment.as_ref(),
            )
            .await;
        }
    };
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            return fail_after_spawn(
                &state.storage,
                request_id,
                &mut child,
                &mut record,
                "child stderr pipe was unavailable",
                previous_environment.as_ref(),
            )
            .await;
        }
    };
    #[cfg(target_os = "linux")]
    let identity = match process_identity::read(pid) {
        Ok(identity) => identity,
        Err(error) => {
            return fail_after_spawn(
                &state.storage,
                request_id,
                &mut child,
                &mut record,
                &format!("could not establish process identity: {error}"),
                previous_environment.as_ref(),
            )
            .await;
        }
    };
    #[cfg(target_os = "linux")]
    if identity.process_group_id != pid || identity.session_id != pid {
        return fail_after_spawn(
            &state.storage,
            request_id,
            &mut child,
            &mut record,
            "spawned child did not establish its own session and process group",
            previous_environment.as_ref(),
        )
        .await;
    }
    #[cfg(target_os = "linux")]
    let process_start_time = Some(identity.start_time);
    #[cfg(not(target_os = "linux"))]
    let process_start_time = None;
    if let Err(error) = record.mark_running(epoch_seconds(), pid, Some(pid), process_start_time) {
        let _ = child.kill().await;
        return persist_start_failure(
            &state.storage,
            request_id,
            &mut record,
            format!("could not mark process running: {error}"),
            previous_environment.as_ref(),
        );
    }
    if let Err(error) = state.storage.save_record(&record) {
        let _ = kill_process_group(pid);
        let _ = child.kill().await;
        let reason = format!("could not persist running process: {error}");
        let _ = record.mark_spawn_failed(epoch_seconds(), reason.clone());
        if let Some(environment) = previous_environment {
            record.set_environment(environment.clone());
        }
        let _ = state.storage.save_record(&record);
        return IpcResponse::error(request_id, ResultStatus::Failure, reason);
    }

    let response = match record_value(&record) {
        Ok(value) => IpcResponse::success(request_id, Some(value)),
        Err(error) => IpcResponse::error(request_id, ResultStatus::Failure, error.to_string()),
    };
    let storage = state.storage.clone();
    tokio::spawn(async move {
        monitor::monitor_child(storage, key, child, stdout, stderr).await;
    });
    response
}

fn duplicate_response(request_id: u64) -> IpcResponse {
    IpcResponse::error(
        request_id,
        ResultStatus::DuplicateRecord,
        "a process with this name already exists in the project",
    )
}

fn create_logs_for_launch(
    storage: &crate::storage::Storage,
    key: &ProcessKey,
) -> Result<crate::process::LogPaths, crate::storage::StorageError> {
    match storage.create_logs(key) {
        Ok(logs) => Ok(logs),
        Err(error @ crate::storage::StorageError::RecordExists { .. }) => {
            if storage.load_record(key)?.is_some() {
                return Err(error);
            }
            storage.remove_logs(key)?;
            storage.create_logs(key)
        }
        Err(error) => Err(error),
    }
}

fn validate_executable(
    working_directory: &Path,
    executable: &std::ffi::OsStr,
    environment: &ResolvedEnvironment,
) -> io::Result<()> {
    let path = Path::new(executable);
    if path.components().count() > 1 {
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            working_directory.join(path)
        };
        return validate_executable_path(&path);
    }
    let search_path = environment
        .entries()
        .iter()
        .find(|entry| entry.key == "PATH")
        .map(|entry| entry.value.clone())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "PATH is not set"))?;
    let path = std::env::split_paths(&search_path)
        .map(|directory| directory.join(path))
        .map(|candidate| {
            if candidate.is_absolute() {
                candidate
            } else {
                working_directory.join(candidate)
            }
        })
        .find(|candidate| validate_executable_path(candidate).is_ok())
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "executable was not found in PATH")
        })?;
    validate_executable_path(&path)
}

fn validate_executable_path(path: &Path) -> io::Result<()> {
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "executable is not a regular file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "executable lacks execute permission",
            ));
        }
    }
    Ok(())
}

fn spawn_child(
    working_directory: &Path,
    executable: &std::ffi::OsStr,
    arguments: &[OsString],
    environment: &ResolvedEnvironment,
) -> io::Result<Child> {
    #[cfg(target_os = "linux")]
    let mut command = {
        let supervisor = std::env::current_exe()?;
        let mut command = Command::new(supervisor);
        command
            .arg(INTERNAL_SUPERVISOR_ARGUMENT)
            .arg(std::process::id().to_string())
            .arg("--")
            .arg(executable)
            .args(arguments);
        command
    };
    #[cfg(not(target_os = "linux"))]
    let mut command = {
        let mut command = Command::new(executable);
        command.args(arguments);
        command
    };
    command
        .current_dir(working_directory)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    environment.apply_to_command(&mut command);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        // The child becomes the leader of its own session and process group.
        unsafe {
            command
                .as_std_mut()
                .pre_exec(|| nix::unistd::setsid().map(|_| ()).map_err(io::Error::other));
        }
    }
    command.spawn()
}

async fn fail_after_spawn(
    storage: &crate::storage::Storage,
    request_id: u64,
    child: &mut Child,
    record: &mut ProcessRecord,
    reason: &str,
    previous_environment: Option<&crate::environment::EnvironmentSpec>,
) -> IpcResponse {
    if let Some(pid) = child.id() {
        let _ = kill_process_group(pid);
    }
    let _ = child.kill().await;
    persist_start_failure(storage, request_id, record, reason, previous_environment)
}

fn persist_start_failure(
    storage: &crate::storage::Storage,
    request_id: u64,
    record: &mut ProcessRecord,
    reason: impl Into<String>,
    previous_environment: Option<&crate::environment::EnvironmentSpec>,
) -> IpcResponse {
    let reason = reason.into();
    let result = match record.mark_spawn_failed(epoch_seconds(), reason.clone()) {
        Ok(()) => {
            if let Some(environment) = previous_environment {
                record.set_environment(environment.clone());
            }
            storage
                .save_record(record)
                .map_err(|error| error.to_string())
        }
        Err(error) => Err(error.to_string()),
    };
    match result {
        Ok(()) => IpcResponse::error(request_id, ResultStatus::Failure, reason),
        Err(error) => IpcResponse::error(
            request_id,
            ResultStatus::Failure,
            format!("{reason}; could not persist failed record: {error}"),
        ),
    }
}

fn kill_process_group(pid: u32) -> nix::Result<()> {
    let pid = i32::try_from(pid)
        .ok()
        .filter(|pid| *pid > 0)
        .ok_or(nix::errno::Errno::EINVAL)?;
    killpg(Pid::from_raw(pid), Signal::SIGKILL)
}

fn storage_failure(request_id: u64, message: String) -> IpcResponse {
    IpcResponse::error(request_id, ResultStatus::Failure, message)
}
