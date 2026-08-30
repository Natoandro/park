use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::process::{ProcessKey, ProcessRecord};
use crate::project::ProjectPath;
use crate::result::{CommandResult, ResultStatus};

pub const PROTOCOL_VERSION: u16 = 1;
const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpcRequest {
    pub version: u16,
    pub request_id: u64,
    #[serde(flatten)]
    pub operation: IpcOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum IpcOperation {
    Ping,
    Ps { project_path: ProjectPath },
    Status { key: ProcessKey },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IpcResponse {
    pub version: u16,
    pub request_id: u64,
    pub result: CommandResult<serde_json::Value>,
}

impl IpcResponse {
    pub fn success(request_id: u64, data: Option<serde_json::Value>) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            request_id,
            result: CommandResult::success(data, None),
        }
    }

    pub fn error(request_id: u64, status: ResultStatus, message: impl Into<String>) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            request_id,
            result: CommandResult::error(status, message),
        }
    }
}

pub async fn send_request(
    socket_path: &Path,
    request: &IpcRequest,
) -> Result<IpcResponse, IpcError> {
    let mut stream = UnixStream::connect(socket_path)
        .await
        .map_err(|source| IpcError::Io {
            operation: "connect to daemon",
            source,
        })?;
    let mut payload = serde_json::to_vec(request).map_err(IpcError::Serialize)?;
    payload.push(b'\n');
    stream
        .write_all(&payload)
        .await
        .map_err(|source| IpcError::Io {
            operation: "send daemon request",
            source,
        })?;
    stream.shutdown().await.map_err(|source| IpcError::Io {
        operation: "finish daemon request",
        source,
    })?;

    let mut reader = BufReader::new(stream);
    let mut line = Vec::new();
    let bytes = reader
        .read_until(b'\n', &mut line)
        .await
        .map_err(|source| IpcError::Io {
            operation: "read daemon response",
            source,
        })?;
    if bytes == 0 {
        return Err(IpcError::Protocol(
            "daemon closed the connection".to_owned(),
        ));
    }
    if line.len() > MAX_MESSAGE_BYTES {
        return Err(IpcError::Protocol(
            "daemon response is too large".to_owned(),
        ));
    }
    if line.last() == Some(&b'\n') {
        line.pop();
    }
    serde_json::from_slice(&line).map_err(IpcError::Deserialize)
}

pub async fn read_request(stream: &mut tokio::net::UnixStream) -> Result<IpcRequest, IpcError> {
    let mut reader = BufReader::new(&mut *stream);
    let mut line = Vec::new();
    let bytes = reader
        .read_until(b'\n', &mut line)
        .await
        .map_err(|source| IpcError::Io {
            operation: "read daemon request",
            source,
        })?;
    if bytes == 0 {
        return Err(IpcError::Protocol(
            "client closed the connection".to_owned(),
        ));
    }
    if line.len() > MAX_MESSAGE_BYTES {
        return Err(IpcError::Protocol("daemon request is too large".to_owned()));
    }
    if line.last() == Some(&b'\n') {
        line.pop();
    }
    serde_json::from_slice(&line).map_err(IpcError::Deserialize)
}

pub async fn write_response(
    stream: &mut UnixStream,
    response: &IpcResponse,
) -> Result<(), IpcError> {
    let mut payload = serde_json::to_vec(response).map_err(IpcError::Serialize)?;
    payload.push(b'\n');
    stream
        .write_all(&payload)
        .await
        .map_err(|source| IpcError::Io {
            operation: "send daemon response",
            source,
        })
}

pub fn request_for_ps(request_id: u64, project_path: ProjectPath) -> IpcRequest {
    IpcRequest {
        version: PROTOCOL_VERSION,
        request_id,
        operation: IpcOperation::Ps { project_path },
    }
}

pub fn request_for_status(request_id: u64, key: ProcessKey) -> IpcRequest {
    IpcRequest {
        version: PROTOCOL_VERSION,
        request_id,
        operation: IpcOperation::Status { key },
    }
}

pub fn record_value(record: &ProcessRecord) -> Result<serde_json::Value, IpcError> {
    serde_json::to_value(record).map_err(IpcError::Serialize)
}

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("could not {operation}: {source}")]
    Io {
        operation: &'static str,
        source: std::io::Error,
    },
    #[error("could not serialize IPC payload: {0}")]
    Serialize(#[source] serde_json::Error),
    #[error("could not decode IPC payload: {0}")]
    Deserialize(#[source] serde_json::Error),
    #[error("invalid IPC protocol message: {0}")]
    Protocol(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn serializes_versioned_status_request() {
        let project = ProjectPath::from_canonical(PathBuf::from("/project"));
        let request = request_for_status(7, ProcessKey::new(project, OsString::from("dev")));
        assert_eq!(
            serde_json::to_string(&request).expect("request should serialize"),
            r#"{"version":1,"request_id":7,"operation":"status","key":{"project_path":"/project","name":"646576"}}"#
        );
    }

    #[test]
    fn serializes_structured_error_response() {
        let response = IpcResponse::error(3, ResultStatus::MissingRecord, "not found");
        assert_eq!(
            serde_json::to_string(&response).expect("response should serialize"),
            r#"{"version":1,"request_id":3,"result":{"status":"missing_record","ok":false,"error":{"code":"missing_record","message":"not found"}}}"#
        );
    }
}
