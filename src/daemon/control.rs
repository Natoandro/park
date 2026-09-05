use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::time::Duration;

use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use tokio::time::{Instant, sleep};

use crate::ipc::{IpcOperation, IpcResponse, record_value};
use crate::lifecycle::{LifecycleAction, ProcessState};
use crate::process::{ProcessKey, ProcessRecord};
use crate::result::ResultStatus;
use crate::storage::StorageError;

use super::{DaemonState, epoch_seconds, launch, process_identity, record_is_alive};

const STOP_TIMEOUT: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(25);

pub(super) async fn handle(
    state: &DaemonState,
    request_id: u64,
    operation: IpcOperation,
) -> IpcResponse {
    match operation {
        IpcOperation::Stop { key, force } => stop(state, request_id, key, force).await,
        IpcOperation::Signal { key, signal: name } => signal(state, request_id, key, &name).await,
        IpcOperation::Restart { key, recapture } => {
            restart(state, request_id, key, recapture).await
        }
        IpcOperation::Start {
            key,
            command,
            environment,
        } => start(state, request_id, key, command, environment).await,
        IpcOperation::Env { key, set, unset } => {
            environment(state, request_id, key, set, unset).await
        }
        IpcOperation::Remove { key, keep_logs } => remove(state, request_id, key, keep_logs).await,
        IpcOperation::Clean => clean(state, request_id).await,
        _ => IpcResponse::error(
            request_id,
            ResultStatus::Failure,
            "unsupported lifecycle operation",
        ),
    }
}

async fn stop(state: &DaemonState, request_id: u64, key: ProcessKey, force: bool) -> IpcResponse {
    let lock = state.lifecycle_lock(&key);
    let _guard = lock.lock().await;
    let Some(record) = load_record(state, request_id, &key) else {
        return missing_or_failure(state, request_id, &key);
    };
    match stop_record(state, request_id, record, force).await {
        Ok(record) => record_response(request_id, record),
        Err(response) => response,
    }
}

async fn stop_record(
    state: &DaemonState,
    request_id: u64,
    mut record: ProcessRecord,
    force: bool,
) -> Result<ProcessRecord, IpcResponse> {
    if let Err(error) = record.state().validate_action(LifecycleAction::Stop) {
        return Err(IpcResponse::error(
            request_id,
            ResultStatus::InvalidState,
            error.to_string(),
        ));
    }
    if !record_is_alive(&record) {
        reconcile(state, request_id)?;
        return load_record(state, request_id, record.key())
            .ok_or_else(|| missing_or_failure(state, request_id, record.key()));
    }
    record
        .transition_to(ProcessState::Stopping)
        .map_err(|error| {
            IpcResponse::error(request_id, ResultStatus::InvalidState, error.to_string())
        })?;
    state
        .storage
        .save_record(&record)
        .map_err(|error| storage_response(request_id, error))?;

    let Some(group_id) = record.process_group_id() else {
        return Err(IpcResponse::error(
            request_id,
            ResultStatus::Failure,
            "running process is missing its process group",
        ));
    };
    let signal = if force {
        Signal::SIGKILL
    } else {
        Signal::SIGTERM
    };
    if let Err(error) = signal_group(group_id, signal) {
        if error == nix::errno::Errno::ESRCH {
            reconcile(state, request_id)?;
            return load_record(state, request_id, record.key())
                .ok_or_else(|| missing_or_failure(state, request_id, record.key()));
        }
        return Err(IpcResponse::error(
            request_id,
            ResultStatus::Failure,
            format!("could not send {signal:?} to process group: {error}"),
        ));
    }

    let deadline = Instant::now() + STOP_TIMEOUT;
    if !force {
        if let Some(record) = wait_for_terminal(state, request_id, record.key(), deadline).await? {
            return Ok(record);
        }
        if let Ok(Some(current)) = state.storage.load_record(record.key())
            && process_identity::owns_group(&current)
            && let Some(group_id) = current.process_group_id()
        {
            let _ = signal_group(group_id, Signal::SIGKILL);
        }
    }
    let deadline = Instant::now() + STOP_TIMEOUT;
    wait_for_terminal(state, request_id, record.key(), deadline)
        .await?
        .ok_or_else(|| {
            IpcResponse::error(
                request_id,
                ResultStatus::Failure,
                "process did not reach a terminal state after termination",
            )
        })
}

async fn signal(state: &DaemonState, request_id: u64, key: ProcessKey, name: &str) -> IpcResponse {
    let lock = state.lifecycle_lock(&key);
    let _guard = lock.lock().await;
    let Some(signal) = parse_signal(name) else {
        return IpcResponse::error(
            request_id,
            ResultStatus::Failure,
            "unsupported signal; use HUP, INT, QUIT, TERM, USR1, USR2, STOP, CONT, or KILL",
        );
    };
    let Some(record) = load_record(state, request_id, &key) else {
        return missing_or_failure(state, request_id, &key);
    };
    if let Err(error) = record.state().validate_action(LifecycleAction::Signal) {
        return IpcResponse::error(request_id, ResultStatus::InvalidState, error.to_string());
    }
    let Some(group_id) = record.process_group_id() else {
        return IpcResponse::error(
            request_id,
            ResultStatus::Failure,
            "active process is missing its process group",
        );
    };
    if !record_is_alive(&record) {
        if let Err(response) = reconcile(state, request_id) {
            return response;
        }
        return load_record(state, request_id, &key).map_or_else(
            || missing_or_failure(state, request_id, &key),
            |record| {
                IpcResponse::error(
                    request_id,
                    ResultStatus::InvalidState,
                    format!("cannot signal process while it is {:?}", record.state()),
                )
            },
        );
    }
    match signal_group(group_id, signal) {
        Ok(()) => record_response(request_id, record),
        Err(error) => IpcResponse::error(
            request_id,
            ResultStatus::Failure,
            format!("could not send {signal:?} to process group: {error}"),
        ),
    }
}

async fn restart(
    state: &DaemonState,
    request_id: u64,
    key: ProcessKey,
    recapture: Option<crate::ipc::RecaptureEnvironment>,
) -> IpcResponse {
    let lock = state.lifecycle_lock(&key);
    let _guard = lock.lock().await;
    let Some(mut record) = load_record(state, request_id, &key) else {
        return missing_or_failure(state, request_id, &key);
    };
    let candidate_environment = recapture.map_or_else(
        || record.environment().clone(),
        |recapture| crate::environment::EnvironmentSpec {
            capture: recapture.capture,
            dotenv_files: recapture
                .dotenv_files
                .unwrap_or_else(|| record.environment().dotenv_files.clone()),
            overrides: record.environment().overrides.clone(),
        },
    );
    if let Err(error) = candidate_environment.resolve(record.working_directory()) {
        return IpcResponse::error(
            request_id,
            ResultStatus::Failure,
            format!("could not resolve process environment: {error}"),
        );
    }
    if record.state() == ProcessState::Running {
        match stop_record(state, request_id, record, false).await {
            Ok(stopped) => record = stopped,
            Err(response) => return response,
        }
    } else if record.state() != ProcessState::Exited
        && record.state() != ProcessState::Failed
        && record.state() != ProcessState::Killed
    {
        return IpcResponse::error(
            request_id,
            ResultStatus::InvalidState,
            format!("cannot restart process while it is {:?}", record.state()),
        );
    }
    relaunch(state, request_id, record, candidate_environment).await
}

async fn start(
    state: &DaemonState,
    request_id: u64,
    key: ProcessKey,
    command: Option<Vec<OsString>>,
    environment: Option<crate::environment::EnvironmentSpec>,
) -> IpcResponse {
    if let Some(command) = command {
        let Some(environment) = environment else {
            return IpcResponse::error(
                request_id,
                ResultStatus::Failure,
                "new start requests require environment inputs",
            );
        };
        return launch::start(
            state,
            request_id,
            crate::project::ProjectPath::from_canonical(key.project_path().to_path_buf()),
            key.name().to_os_string(),
            command,
            environment,
        )
        .await;
    }
    let lock = state.lifecycle_lock(&key);
    let _guard = lock.lock().await;
    let Some(record) = load_record(state, request_id, &key) else {
        return missing_or_failure(state, request_id, &key);
    };
    if let Err(error) = record.state().validate_action(LifecycleAction::Start) {
        return IpcResponse::error(request_id, ResultStatus::InvalidState, error.to_string());
    }
    let environment = record.environment().clone();
    relaunch(state, request_id, record, environment).await
}

async fn relaunch(
    state: &DaemonState,
    request_id: u64,
    mut record: ProcessRecord,
    environment: crate::environment::EnvironmentSpec,
) -> IpcResponse {
    let previous_environment = record.environment().clone();
    record.set_environment(environment);
    if let Err(error) = record.reset_for_start() {
        return IpcResponse::error(request_id, ResultStatus::InvalidState, error.to_string());
    }
    if let Err(error) = state.storage.save_record(&record) {
        return storage_response(request_id, error);
    }
    launch::spawn_record_with_previous(state, request_id, record, Some(previous_environment)).await
}

async fn environment(
    state: &DaemonState,
    request_id: u64,
    key: ProcessKey,
    set: Vec<OsString>,
    unset: Vec<OsString>,
) -> IpcResponse {
    let lock = state.lifecycle_lock(&key);
    let _guard = lock.lock().await;
    let Some(mut record) = load_record(state, request_id, &key) else {
        return missing_or_failure(state, request_id, &key);
    };
    let mutated = !set.is_empty() || !unset.is_empty();
    let mut inputs = record.environment().clone();
    for value in set {
        let Some((key, value)) = split_assignment(&value) else {
            return IpcResponse::error(
                request_id,
                ResultStatus::Failure,
                "--set requires KEY=VALUE",
            );
        };
        if let Err(error) = inputs.set(key, value) {
            return IpcResponse::error(request_id, ResultStatus::Failure, error.to_string());
        }
    }
    for key in unset {
        if let Err(error) = inputs.unset(key) {
            return IpcResponse::error(request_id, ResultStatus::Failure, error.to_string());
        }
    }
    if mutated {
        record.set_environment(inputs);
        if let Err(error) = state.storage.save_record(&record) {
            return storage_response(request_id, error);
        }
    }
    match record.environment().resolve(record.working_directory()) {
        Ok(environment) => IpcResponse::success(request_id, Some(environment.display_value())),
        Err(error) => IpcResponse::error(request_id, ResultStatus::Failure, error.to_string()),
    }
}

fn split_assignment(value: &OsString) -> Option<(OsString, OsString)> {
    let bytes = value.as_bytes();
    let separator = bytes.iter().position(|byte| *byte == b'=')?;
    Some((
        OsString::from_vec(bytes[..separator].to_vec()),
        OsString::from_vec(bytes[separator + 1..].to_vec()),
    ))
}

async fn remove(
    state: &DaemonState,
    request_id: u64,
    key: ProcessKey,
    keep_logs: bool,
) -> IpcResponse {
    let lock = state.lifecycle_lock(&key);
    let _guard = lock.lock().await;
    let Some(record) = load_record(state, request_id, &key) else {
        return missing_or_failure(state, request_id, &key);
    };
    if process_identity::owns_group(&record) {
        return IpcResponse::error(
            request_id,
            ResultStatus::InvalidState,
            "managed process group is still active",
        );
    }
    match state.storage.remove_record(&key, keep_logs) {
        Ok(()) => IpcResponse::success(request_id, None),
        Err(error) => storage_response(request_id, error),
    }
}

async fn clean(state: &DaemonState, request_id: u64) -> IpcResponse {
    let records = match state.storage.list_records() {
        Ok(records) => records,
        Err(error) => return storage_response(request_id, error),
    };
    let mut removed = 0_u64;
    for record in records {
        if !record.state().is_terminal() || process_identity::owns_group(&record) {
            continue;
        }
        let key = record.key().clone();
        let lock = state.lifecycle_lock(&key);
        let _guard = lock.lock().await;
        match state.storage.remove_record(&key, false) {
            Ok(()) => removed += 1,
            Err(StorageError::ActiveRecord { .. } | StorageError::RecordMissing { .. }) => {}
            Err(error) => return storage_response(request_id, error),
        }
    }
    IpcResponse::success(request_id, Some(serde_json::json!({"removed": removed})))
}

async fn wait_for_terminal(
    state: &DaemonState,
    request_id: u64,
    key: &ProcessKey,
    deadline: Instant,
) -> Result<Option<ProcessRecord>, IpcResponse> {
    loop {
        let record = state
            .storage
            .load_record(key)
            .map_err(|error| storage_response(request_id, error))?;
        let Some(record) = record else {
            return Ok(None);
        };
        if record.state().is_terminal() && !process_identity::owns_group(&record) {
            return Ok(Some(record));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        sleep(POLL_INTERVAL).await;
    }
}

fn load_record(state: &DaemonState, _request_id: u64, key: &ProcessKey) -> Option<ProcessRecord> {
    state.storage.load_record(key).ok().flatten()
}

fn reconcile(state: &DaemonState, request_id: u64) -> Result<(), IpcResponse> {
    state
        .storage
        .reconcile(epoch_seconds(), record_is_alive)
        .map(|_| ())
        .map_err(|error| storage_response(request_id, error))
}

fn signal_group(group_id: u32, signal: Signal) -> nix::Result<()> {
    let group_id = i32::try_from(group_id)
        .ok()
        .filter(|group_id| *group_id > 0)
        .ok_or(nix::errno::Errno::EINVAL)?;
    killpg(Pid::from_raw(group_id), signal)
}

fn parse_signal(name: &str) -> Option<Signal> {
    let normalized = name.trim().to_ascii_uppercase();
    match normalized.strip_prefix("SIG").unwrap_or(&normalized) {
        "HUP" => Some(Signal::SIGHUP),
        "INT" => Some(Signal::SIGINT),
        "QUIT" => Some(Signal::SIGQUIT),
        "TERM" => Some(Signal::SIGTERM),
        "USR1" => Some(Signal::SIGUSR1),
        "USR2" => Some(Signal::SIGUSR2),
        "STOP" => Some(Signal::SIGSTOP),
        "CONT" => Some(Signal::SIGCONT),
        "KILL" => Some(Signal::SIGKILL),
        _ => None,
    }
}

fn record_response(request_id: u64, record: ProcessRecord) -> IpcResponse {
    match record_value(&record) {
        Ok(value) => IpcResponse::success(request_id, Some(value)),
        Err(error) => IpcResponse::error(request_id, ResultStatus::Failure, error.to_string()),
    }
}

fn missing_or_failure(state: &DaemonState, request_id: u64, key: &ProcessKey) -> IpcResponse {
    match state.storage.load_record(key) {
        Ok(Some(_)) => IpcResponse::error(
            request_id,
            ResultStatus::Failure,
            "could not load process record",
        ),
        Ok(None) => IpcResponse::error(
            request_id,
            ResultStatus::MissingRecord,
            "no process record exists",
        ),
        Err(error) => storage_response(request_id, error),
    }
}

fn storage_response(request_id: u64, error: StorageError) -> IpcResponse {
    let status = match &error {
        StorageError::RecordMissing { .. } => ResultStatus::MissingRecord,
        StorageError::ActiveRecord { .. } => ResultStatus::InvalidState,
        StorageError::RecordExists { .. } => ResultStatus::DuplicateRecord,
        _ => ResultStatus::Failure,
    };
    IpcResponse::error(request_id, status, error.to_string())
}
