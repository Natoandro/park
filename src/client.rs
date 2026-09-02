use std::io;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use thiserror::Error;
use tokio::time::sleep;

use crate::daemon::INTERNAL_DAEMON_ARGUMENT;
use crate::ipc::{
    IpcError, IpcRequest, IpcResponse, request_for_handshake, send_request, send_stream_request,
};
use crate::storage::{StorageError, StoragePaths};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(3);
const RETRY_INTERVAL: Duration = Duration::from_millis(25);
const HANDSHAKE_REQUEST_ID: u64 = 0;

pub async fn request_with_daemon_start(
    paths: &StoragePaths,
    request: &IpcRequest,
) -> Result<IpcResponse, ClientError> {
    paths.ensure_directories()?;
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    let mut started = false;

    loop {
        let last_error = match send_handshake(&paths.socket_path()).await {
            Ok(()) => match send_request(&paths.socket_path(), request).await {
                Ok(response) => return Ok(response),
                Err(error) => error,
            },
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
        let last_error = match send_handshake(&paths.socket_path()).await {
            Ok(()) => match send_stream_request(&paths.socket_path(), request, &mut on_chunk).await
            {
                Ok(response) => return Ok(response),
                Err(error) => error,
            },
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

async fn send_handshake(socket_path: &std::path::Path) -> Result<(), IpcError> {
    let client_version = env!("CARGO_PKG_VERSION").to_owned();
    let request = request_for_handshake(HANDSHAKE_REQUEST_ID, client_version.clone());
    let response = send_request(socket_path, &request).await?;
    validate_handshake(response, &client_version)
}

fn validate_handshake(response: IpcResponse, client_version: &str) -> Result<(), IpcError> {
    if !response.result.ok {
        return Err(IpcError::Handshake(
            response.result.human_message().to_owned(),
        ));
    }
    let data = response.result.data.as_ref().ok_or_else(|| {
        IpcError::Handshake("daemon response is missing handshake data".to_owned())
    })?;
    let echoed_client_version = data
        .get("client_version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            IpcError::Handshake("daemon response is missing its client version".to_owned())
        })?;
    if echoed_client_version != client_version {
        return Err(IpcError::Handshake(
            "daemon response contains the wrong client version".to_owned(),
        ));
    }
    data.get("daemon_version")
        .and_then(serde_json::Value::as_str)
        .filter(|version| !version.is_empty())
        .ok_or_else(|| IpcError::Handshake("daemon response is missing its version".to_owned()))?;
    Ok(())
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

    #[test]
    fn validates_a_successful_daemon_handshake() {
        let response = IpcResponse::success(
            HANDSHAKE_REQUEST_ID,
            Some(serde_json::json!({
                "client_version": "0.2.1",
                "daemon_version": "0.2.1"
            })),
        );
        assert!(validate_handshake(response, "0.2.1").is_ok());
    }

    #[test]
    fn rejects_a_handshake_without_a_daemon_version() {
        let response = IpcResponse::success(HANDSHAKE_REQUEST_ID, None);
        assert!(matches!(
            validate_handshake(response, "0.2.1"),
            Err(IpcError::Handshake(message)) if message.contains("handshake data")
        ));
    }

    #[test]
    fn rejects_a_handshake_that_echoes_the_wrong_client_version() {
        let response = IpcResponse::success(
            HANDSHAKE_REQUEST_ID,
            Some(serde_json::json!({
                "client_version": "0.1.0",
                "daemon_version": "0.2.1"
            })),
        );
        assert!(matches!(
            validate_handshake(response, "0.2.1"),
            Err(IpcError::Handshake(message)) if message.contains("wrong client version")
        ));
    }
}
