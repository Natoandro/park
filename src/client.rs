use std::io;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use thiserror::Error;
use tokio::time::sleep;

use crate::daemon::INTERNAL_DAEMON_ARGUMENT;
use crate::ipc::{IpcError, IpcRequest, IpcResponse, send_request, send_stream_request};
use crate::storage::{StorageError, StoragePaths};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(3);
const RETRY_INTERVAL: Duration = Duration::from_millis(25);

pub async fn request_with_daemon_start(
    paths: &StoragePaths,
    request: &IpcRequest,
) -> Result<IpcResponse, ClientError> {
    paths.ensure_directories()?;
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    let mut started = false;

    loop {
        let last_error = match send_request(&paths.socket_path(), request).await {
            Ok(response) => return Ok(response),
            Err(error) => error,
        };

        if !started && should_start_daemon(&last_error) {
            spawn_daemon()?;
            started = true;
        }
        if !started || !should_start_daemon(&last_error) {
            return Err(last_error.into());
        }
        if Instant::now() >= deadline {
            return Err(ClientError::StartupTimeout { source: last_error });
        }
        sleep(RETRY_INTERVAL).await;
    }
}

pub async fn stream_request_with_daemon_start<F>(
    paths: &StoragePaths,
    request: &IpcRequest,
    on_chunk: F,
) -> Result<IpcResponse, ClientError>
where
    F: FnMut(&str),
{
    paths.ensure_directories()?;
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    let mut started = false;
    let mut on_chunk = on_chunk;

    loop {
        let last_error =
            match send_stream_request(&paths.socket_path(), request, &mut on_chunk).await {
                Ok(response) => return Ok(response),
                Err(error) => error,
            };

        if !started && should_start_daemon(&last_error) {
            spawn_daemon()?;
            started = true;
        }
        if !started || !should_start_daemon(&last_error) {
            return Err(last_error.into());
        }
        if Instant::now() >= deadline {
            return Err(ClientError::StartupTimeout { source: last_error });
        }
        sleep(RETRY_INTERVAL).await;
    }
}

fn should_start_daemon(error: &IpcError) -> bool {
    matches!(
        error,
        IpcError::Io { source, .. }
            if matches!(source.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused)
    )
}

fn spawn_daemon() -> Result<(), ClientError> {
    let executable = std::env::current_exe().map_err(|source| ClientError::Io {
        operation: "locate park executable",
        source,
    })?;
    let mut command = Command::new(executable);
    command
        .arg(INTERNAL_DAEMON_ARGUMENT)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        // A daemon must not retain the invoking terminal's session or process group.
        unsafe {
            command.pre_exec(|| nix::unistd::setsid().map(|_| ()).map_err(io::Error::other));
        }
    }

    command
        .spawn()
        .map(|_| ())
        .map_err(|source| ClientError::Io {
            operation: "start daemon",
            source,
        })
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Ipc(#[from] IpcError),
    #[error("could not {operation}: {source}")]
    Io {
        operation: &'static str,
        source: io::Error,
    },
    #[error("daemon did not become ready: {source}")]
    StartupTimeout { source: IpcError },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connection_error(kind: io::ErrorKind) -> IpcError {
        IpcError::Io {
            operation: "connect to daemon",
            source: io::Error::from(kind),
        }
    }

    #[test]
    fn starts_only_for_missing_or_refused_daemon_sockets() {
        assert!(should_start_daemon(&connection_error(
            io::ErrorKind::NotFound
        )));
        assert!(should_start_daemon(&connection_error(
            io::ErrorKind::ConnectionRefused
        )));
        assert!(!should_start_daemon(&connection_error(
            io::ErrorKind::PermissionDenied
        )));
        assert!(!should_start_daemon(&IpcError::Protocol(
            "bad response".to_owned()
        )));
    }
}
