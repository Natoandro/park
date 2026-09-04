use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use nix::errno::Errno;
use nix::fcntl::{Flock, FlockArg};
use thiserror::Error;
use tokio::net::{UnixListener, UnixStream};
use tokio::time::{Duration, timeout};

use crate::config::{ConfigSource, LoadedConfig};
use crate::ipc::{
    IpcError, IpcOperation, IpcRequest, IpcResponse, PROTOCOL_VERSION, read_request, record_value,
    write_response,
};
use crate::process::{ProcessKey, ProcessRecord, validate_process_name};
use crate::project::resolve_project;
use crate::result::ResultStatus;
use crate::storage::{Storage, StorageError, StoragePaths};

mod control;
pub mod descriptors;
pub mod handoff;
mod launch;
mod logs;
mod monitor;
mod process_identity;
mod wait;

pub const INTERNAL_DAEMON_ARGUMENT: &str = "--internal-daemon";
pub const INTERNAL_SUPERVISOR_ARGUMENT: &str = "--internal-supervisor";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
use handoff::HANDOFF_VERSION;
const DAEMON_GENERATION: u64 = 1;
const REEXEC_STATE: &str = "serving";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonPhase {
    Serving,
    Quiescing,
    HandingOff,
}

impl DaemonPhase {
    fn as_u8(self) -> u8 {
        match self {
            Self::Serving => 0,
            Self::Quiescing => 1,
            Self::HandingOff => 2,
        }
    }

    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Quiescing,
            2 => Self::HandingOff,
            _ => Self::Serving,
        }
    }
}

#[derive(Debug)]
pub struct DaemonLock {
    _file: Flock<File>,
    paths: StoragePaths,
}

impl DaemonLock {
    pub fn try_acquire(paths: StoragePaths) -> Result<Option<Self>, DaemonError> {
        paths.ensure_directories()?;
        let lock_path = paths.lock_path();
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(|source| DaemonError::Io {
                operation: "open daemon lock",
                path: lock_path.clone(),
                source,
            })?;
        set_private_file_permissions(&lock_path)?;

        match Flock::lock(file, FlockArg::LockExclusiveNonblock) {
            Ok(file) => Ok(Some(Self { _file: file, paths })),
            Err((_file, error)) if error == Errno::EAGAIN || error == Errno::EWOULDBLOCK => {
                Ok(None)
            }
            Err((_file, source)) => Err(DaemonError::Lock {
                path: lock_path,
                source,
            }),
        }
    }

    fn prepare_endpoint(&self) -> Result<UnixListener, DaemonError> {
        remove_stale_runtime_file(&self.paths.socket_path())?;
        remove_stale_runtime_file(&self.paths.pid_path())?;
        let listener =
            UnixListener::bind(self.paths.socket_path()).map_err(|source| DaemonError::Io {
                operation: "bind daemon socket",
                path: self.paths.socket_path(),
                source,
            })?;
        set_private_file_permissions(&self.paths.socket_path())?;
        fs::write(self.paths.pid_path(), format!("{}\n", std::process::id())).map_err(
            |source| DaemonError::Io {
                operation: "write daemon marker",
                path: self.paths.pid_path(),
                source,
            },
        )?;
        set_private_file_permissions(&self.paths.pid_path())?;
        Ok(listener)
    }
}

impl Drop for DaemonLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.paths.socket_path());
        let _ = fs::remove_file(self.paths.pid_path());
    }
}

pub(super) struct DaemonState {
    pub(super) storage: Storage,
    phase: AtomicU8,
    active_launches: Mutex<std::collections::HashSet<ProcessKey>>,
    lifecycle_locks: Mutex<std::collections::HashMap<ProcessKey, Arc<tokio::sync::Mutex<()>>>>,
}

impl DaemonState {
    pub(super) fn phase(&self) -> DaemonPhase {
        DaemonPhase::from_u8(self.phase.load(Ordering::Acquire))
    }

    #[allow(dead_code)]
    pub(super) fn set_phase(&self, phase: DaemonPhase) {
        self.phase.store(phase.as_u8(), Ordering::Release);
    }

    pub(super) fn reserve_launch(&self, key: ProcessKey) -> Option<LaunchReservation<'_>> {
        let mut active = self
            .active_launches
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !active.insert(key.clone()) {
            return None;
        }
        Some(LaunchReservation {
            active_launches: &self.active_launches,
            key,
        })
    }

    pub(super) fn lifecycle_lock(&self, key: &ProcessKey) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self
            .lifecycle_locks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Arc::clone(
            locks
                .entry(key.clone())
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(()))),
        )
    }
}

pub(super) struct LaunchReservation<'a> {
    active_launches: &'a Mutex<std::collections::HashSet<ProcessKey>>,
    key: ProcessKey,
}

impl Drop for LaunchReservation<'_> {
    fn drop(&mut self) {
        self.active_launches
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&self.key);
    }
}

pub async fn run(paths: StoragePaths) -> Result<bool, DaemonError> {
    let Some(lock) = DaemonLock::try_acquire(paths.clone())? else {
        return Ok(false);
    };
    let listener = lock.prepare_endpoint()?;
    let storage = Storage::new(paths);
    storage.paths().ensure_directories()?;
    let now = epoch_seconds();
    storage.reconcile(now, record_is_alive)?;
    let state = Arc::new(DaemonState {
        storage,
        phase: AtomicU8::new(DaemonPhase::Serving.as_u8()),
        active_launches: Mutex::new(std::collections::HashSet::new()),
        lifecycle_locks: Mutex::new(std::collections::HashMap::new()),
    });

    loop {
        let (stream, _) = listener.accept().await.map_err(|source| DaemonError::Io {
            operation: "accept daemon connection",
            path: state.storage.paths().socket_path(),
            source,
        })?;
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            let _ = serve_connection(stream, state).await;
        });
    }
}

async fn serve_connection(mut stream: UnixStream, state: Arc<DaemonState>) -> Result<(), IpcError> {
    let request_result = timeout(REQUEST_TIMEOUT, read_request(&mut stream)).await;
    let request = match request_result {
        Err(_) => {
            let response = IpcResponse::error(
                0,
                ResultStatus::Failure,
                "timed out waiting for an IPC request",
            );
            return write_response(&mut stream, &response).await;
        }
        Ok(Ok(request)) => request,
        Ok(Err(error)) => {
            let response = IpcResponse::error(0, ResultStatus::Failure, error.to_string());
            return write_response(&mut stream, &response).await;
        }
    };
    let response = dispatch_request(&state, request).await;
    match response {
        DispatchResponse::Single(response) => write_response(&mut stream, &response).await,
        DispatchResponse::Logs {
            request_id,
            key,
            options,
        } => logs::serve(&state, request_id, key, options, &mut stream).await,
        DispatchResponse::Wait {
            request_id,
            key,
            options,
        } => wait::serve(&state, request_id, key, options, &mut stream).await,
    }
}

enum DispatchResponse {
    Single(IpcResponse),
    Logs {
        request_id: u64,
        key: ProcessKey,
        options: logs::LogOptions,
    },
    Wait {
        request_id: u64,
        key: ProcessKey,
        options: wait::WaitOptions,
    },
}

async fn dispatch_request(state: &DaemonState, request: IpcRequest) -> DispatchResponse {
    if request.version != PROTOCOL_VERSION {
        return DispatchResponse::Single(IpcResponse::error(
            request.request_id,
            ResultStatus::Failure,
            format!("unsupported IPC protocol version {}", request.version),
        ));
    }
    if request.client_version != env!("CARGO_PKG_VERSION") {
        return DispatchResponse::Single(incompatible_client_response(
            request.request_id,
            &request.client_version,
        ));
    }

    if state.phase() != DaemonPhase::Serving
        && operation_is_blocked_during_quiesce(&request.operation)
    {
        return DispatchResponse::Single(IpcResponse::daemon_restarting(
            request.request_id,
            DAEMON_GENERATION,
        ));
    }

    let request_id = request.request_id;
    let operation = match canonicalize_operation(request.operation) {
        Ok(operation) => operation,
        Err(error) => {
            return DispatchResponse::Single(IpcResponse::error(
                request_id,
                ResultStatus::Failure,
                error.to_string(),
            ));
        }
    };
    if let Some(name) = operation_name(&operation)
        && let Err(error) = validate_process_name(name)
    {
        return DispatchResponse::Single(IpcResponse::error(
            request_id,
            ResultStatus::Failure,
            error.to_string(),
        ));
    }
    match operation {
        IpcOperation::Launch {
            project_path,
            name,
            command,
        } => DispatchResponse::Single(
            launch::start(state, request_id, project_path, name, command).await,
        ),
        IpcOperation::Logs {
            key,
            tail,
            head,
            follow,
            grep,
            stdout,
            stderr,
        } => DispatchResponse::Logs {
            request_id,
            key,
            options: logs::LogOptions {
                tail,
                head,
                follow,
                grep,
                stdout,
                stderr,
            },
        },
        IpcOperation::Wait {
            key,
            state: expected_state,
            match_text,
            exit,
            timeout_ms,
        } => DispatchResponse::Wait {
            request_id,
            key,
            options: wait::WaitOptions {
                expected_state,
                match_text,
                exit,
                timeout_ms,
            },
        },
        operation @ (IpcOperation::Stop { .. }
        | IpcOperation::Signal { .. }
        | IpcOperation::Restart { .. }
        | IpcOperation::Start { .. }
        | IpcOperation::Remove { .. }
        | IpcOperation::Clean) => {
            DispatchResponse::Single(control::handle(state, request_id, operation).await)
        }
        operation => handle_request(&state.storage, IpcRequest::new(request_id, operation)).into(),
    }
}

fn operation_is_blocked_during_quiesce(operation: &IpcOperation) -> bool {
    !matches!(
        operation,
        IpcOperation::Ping | IpcOperation::DaemonStatus | IpcOperation::DaemonConfig
    )
}

fn operation_name(operation: &IpcOperation) -> Option<&OsStr> {
    match operation {
        IpcOperation::Launch { name, .. } => Some(name),
        IpcOperation::Status { key }
        | IpcOperation::Logs { key, .. }
        | IpcOperation::Wait { key, .. }
        | IpcOperation::Stop { key, .. }
        | IpcOperation::Signal { key, .. }
        | IpcOperation::Restart { key }
        | IpcOperation::Start { key }
        | IpcOperation::Remove { key, .. } => Some(key.name()),
        IpcOperation::Ping
        | IpcOperation::DaemonStatus
        | IpcOperation::DaemonConfig
        | IpcOperation::Reexec { .. }
        | IpcOperation::Ps { .. }
        | IpcOperation::Clean => None,
    }
}

impl From<IpcResponse> for DispatchResponse {
    fn from(response: IpcResponse) -> Self {
        Self::Single(response)
    }
}

fn canonicalize_operation(
    operation: IpcOperation,
) -> Result<IpcOperation, crate::ProjectResolutionError> {
    match operation {
        IpcOperation::Launch {
            project_path,
            name,
            command,
        } => Ok(IpcOperation::Launch {
            project_path: resolve_project(project_path.as_path())?,
            name,
            command,
        }),
        IpcOperation::Ps { project_path } => Ok(IpcOperation::Ps {
            project_path: resolve_project(project_path.as_path())?,
        }),
        IpcOperation::Status { key } => Ok(IpcOperation::Status {
            key: ProcessKey::new(
                resolve_project(key.project_path())?,
                key.name().to_os_string(),
            ),
        }),
        IpcOperation::Logs {
            key,
            tail,
            head,
            follow,
            grep,
            stdout,
            stderr,
        } => Ok(IpcOperation::Logs {
            key: ProcessKey::new(
                resolve_project(key.project_path())?,
                key.name().to_os_string(),
            ),
            tail,
            head,
            follow,
            grep,
            stdout,
            stderr,
        }),
        IpcOperation::Wait {
            key,
            state,
            match_text,
            exit,
            timeout_ms,
        } => Ok(IpcOperation::Wait {
            key: canonical_key(key)?,
            state,
            match_text,
            exit,
            timeout_ms,
        }),
        IpcOperation::Stop { key, force } => Ok(IpcOperation::Stop {
            key: canonical_key(key)?,
            force,
        }),
        IpcOperation::Signal { key, signal } => Ok(IpcOperation::Signal {
            key: canonical_key(key)?,
            signal,
        }),
        IpcOperation::Restart { key } => Ok(IpcOperation::Restart {
            key: canonical_key(key)?,
        }),
        IpcOperation::Start { key } => Ok(IpcOperation::Start {
            key: canonical_key(key)?,
        }),
        IpcOperation::Remove { key, keep_logs } => Ok(IpcOperation::Remove {
            key: canonical_key(key)?,
            keep_logs,
        }),
        IpcOperation::Ping => Ok(IpcOperation::Ping),
        IpcOperation::DaemonStatus => Ok(IpcOperation::DaemonStatus),
        IpcOperation::DaemonConfig => Ok(IpcOperation::DaemonConfig),
        IpcOperation::Reexec {
            candidate_path,
            candidate_version,
        } => Ok(IpcOperation::Reexec {
            candidate_path,
            candidate_version,
        }),
        IpcOperation::Clean => Ok(IpcOperation::Clean),
    }
}

fn canonical_key(key: ProcessKey) -> Result<ProcessKey, crate::ProjectResolutionError> {
    Ok(ProcessKey::new(
        resolve_project(key.project_path())?,
        key.name().to_os_string(),
    ))
}

fn handle_request(storage: &Storage, request: IpcRequest) -> IpcResponse {
    if request.version != PROTOCOL_VERSION {
        return IpcResponse::error(
            request.request_id,
            ResultStatus::Failure,
            format!("unsupported IPC protocol version {}", request.version),
        );
    }
    if request.client_version != env!("CARGO_PKG_VERSION") {
        return incompatible_client_response(request.request_id, &request.client_version);
    }

    if let Err(error) = storage.reconcile(epoch_seconds(), record_is_alive) {
        return storage_error(request.request_id, error);
    }

    match request.operation {
        IpcOperation::Ping => IpcResponse::success(request.request_id, None),
        IpcOperation::DaemonStatus => daemon_status(storage, request.request_id),
        IpcOperation::DaemonConfig => daemon_config(request.request_id),
        IpcOperation::Reexec { .. } => IpcResponse::error(
            request.request_id,
            ResultStatus::Failure,
            "reexec requests are not implemented",
        ),
        IpcOperation::Launch { .. } => IpcResponse::error(
            request.request_id,
            ResultStatus::Failure,
            "launch requests require the daemon dispatcher",
        ),
        IpcOperation::Logs { .. } => IpcResponse::error(
            request.request_id,
            ResultStatus::Failure,
            "log requests require the daemon dispatcher",
        ),
        IpcOperation::Wait { .. } => IpcResponse::error(
            request.request_id,
            ResultStatus::Failure,
            "wait requests require the daemon dispatcher",
        ),
        IpcOperation::Ps { project_path } => {
            let records = match storage.list_records() {
                Ok(records) => records
                    .into_iter()
                    .filter(|record| record.key().project_path() == project_path.as_path())
                    .collect::<Vec<_>>(),
                Err(error) => return storage_error(request.request_id, error),
            };
            let mut records = records;
            records.sort_by(|left, right| {
                use std::os::unix::ffi::OsStrExt;

                left.key()
                    .name()
                    .as_bytes()
                    .cmp(right.key().name().as_bytes())
            });
            let values = records
                .iter()
                .map(record_value)
                .collect::<Result<Vec<_>, _>>();
            match values {
                Ok(values) => {
                    IpcResponse::success(request.request_id, Some(serde_json::Value::Array(values)))
                }
                Err(error) => {
                    IpcResponse::error(request.request_id, ResultStatus::Failure, error.to_string())
                }
            }
        }
        IpcOperation::Status { key } => match storage.load_record(&key) {
            Ok(Some(record)) => match record_value(&record) {
                Ok(value) => IpcResponse::success(request.request_id, Some(value)),
                Err(error) => {
                    IpcResponse::error(request.request_id, ResultStatus::Failure, error.to_string())
                }
            },
            Ok(None) => IpcResponse::error(
                request.request_id,
                ResultStatus::MissingRecord,
                "no process record exists",
            ),
            Err(error) => storage_error(request.request_id, error),
        },
        IpcOperation::Stop { .. }
        | IpcOperation::Signal { .. }
        | IpcOperation::Restart { .. }
        | IpcOperation::Start { .. }
        | IpcOperation::Remove { .. }
        | IpcOperation::Clean => IpcResponse::error(
            request.request_id,
            ResultStatus::Failure,
            "lifecycle requests require the daemon dispatcher",
        ),
    }
}

fn daemon_status(storage: &Storage, request_id: u64) -> IpcResponse {
    let active_record_count = match storage.list_records() {
        Ok(records) => records
            .iter()
            .filter(|record| !record.state().is_terminal())
            .count(),
        Err(error) => return storage_error(request_id, error),
    };
    IpcResponse::success(
        request_id,
        Some(serde_json::json!({
            "pid": std::process::id(),
            "binary_version": env!("CARGO_PKG_VERSION"),
            "protocol_version": PROTOCOL_VERSION,
            "handoff_version": HANDOFF_VERSION,
            "generation": DAEMON_GENERATION,
            "reexec_state": REEXEC_STATE,
            "active_record_count": active_record_count,
        })),
    )
}

fn daemon_config(request_id: u64) -> IpcResponse {
    let environment = crate::storage::XdgEnvironment::from_process();
    let loaded = match crate::config::Config::load_with_source(&environment) {
        Ok(loaded) => loaded,
        Err(error) => {
            return IpcResponse::error(request_id, ResultStatus::Failure, error.to_string());
        }
    };
    match config_value(loaded) {
        Ok(value) => IpcResponse::success(request_id, Some(value)),
        Err(error) => IpcResponse::error(request_id, ResultStatus::Failure, error),
    }
}

fn config_value(loaded: LoadedConfig) -> Result<serde_json::Value, String> {
    let (source, path) = match loaded.source {
        ConfigSource::Defaults => (
            "defaults",
            crate::config::config_path(&crate::storage::XdgEnvironment::from_process()),
        ),
        ConfigSource::File(path) => ("file", Some(path)),
    };
    let config = serde_json::to_value(loaded.config).map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "source": source,
        "path": path.map(|path| path.to_string_lossy().into_owned()),
        "config": config,
    }))
}

fn incompatible_client_response(request_id: u64, client_version: &str) -> IpcResponse {
    IpcResponse::error(
        request_id,
        ResultStatus::Failure,
        format!(
            "incompatible Park versions: client {client_version}, daemon {}",
            env!("CARGO_PKG_VERSION")
        ),
    )
}

fn storage_error(request_id: u64, error: StorageError) -> IpcResponse {
    IpcResponse::error(request_id, ResultStatus::Failure, error.to_string())
}

fn record_is_alive(record: &ProcessRecord) -> bool {
    process_identity::owns_group(record)
}

fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn remove_stale_runtime_file(path: &PathBuf) -> Result<(), DaemonError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(DaemonError::Io {
            operation: "remove stale daemon runtime file",
            path: path.clone(),
            source,
        }),
    }
}

fn set_private_file_permissions(path: &PathBuf) -> Result<(), DaemonError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|source| {
            DaemonError::Io {
                operation: "set daemon file permissions",
                path: path.clone(),
                source,
            }
        })?;
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("could not {operation} {path:?}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    #[error("could not lock daemon state at {path:?}: {source}")]
    Lock { path: PathBuf, source: Errno },
}

#[cfg(test)]
mod tests;
