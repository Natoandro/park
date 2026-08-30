use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use nix::errno::Errno;
use nix::fcntl::{Flock, FlockArg};
use nix::sys::signal::kill;
use nix::unistd::Pid;
use thiserror::Error;
use tokio::net::{UnixListener, UnixStream};

use crate::ipc::{
    IpcError, IpcOperation, IpcRequest, IpcResponse, PROTOCOL_VERSION, read_request, record_value,
    write_response,
};
use crate::process::ProcessRecord;
use crate::result::ResultStatus;
use crate::storage::{Storage, StorageError, StoragePaths};

pub const INTERNAL_DAEMON_ARGUMENT: &str = "--internal-daemon";

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

#[derive(Clone)]
struct DaemonState {
    storage: Storage,
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
    let state = Arc::new(DaemonState { storage });

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
    let request = match read_request(&mut stream).await {
        Ok(request) => request,
        Err(error) => {
            let response = IpcResponse::error(0, ResultStatus::Failure, error.to_string());
            return write_response(&mut stream, &response).await;
        }
    };
    let response = handle_request(&state.storage, request);
    write_response(&mut stream, &response).await
}

fn handle_request(storage: &Storage, request: IpcRequest) -> IpcResponse {
    if request.version != PROTOCOL_VERSION {
        return IpcResponse::error(
            request.request_id,
            ResultStatus::Failure,
            format!("unsupported IPC protocol version {}", request.version),
        );
    }

    match request.operation {
        IpcOperation::Ping => IpcResponse::success(request.request_id, None),
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
                left.key()
                    .name()
                    .to_string_lossy()
                    .cmp(&right.key().name().to_string_lossy())
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
    }
}

fn storage_error(request_id: u64, error: StorageError) -> IpcResponse {
    IpcResponse::error(request_id, ResultStatus::Failure, error.to_string())
}

fn record_is_alive(record: &ProcessRecord) -> bool {
    let Some(pid) = record.pid() else {
        return false;
    };
    match kill(Pid::from_raw(pid as i32), None) {
        Ok(()) => true,
        Err(Errno::EPERM) => true,
        Err(_) => false,
    }
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
