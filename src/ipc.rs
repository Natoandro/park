use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::time::{Duration, timeout};

use crate::lifecycle::ProcessState;
use crate::process::{ProcessKey, ProcessRecord};
use crate::project::ProjectPath;
use crate::result::{CommandResult, ResultStatus};

pub const PROTOCOL_VERSION: u16 = 1;
pub const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpcRequest {
    pub version: u16,
    pub request_id: u64,
    pub client_version: String,
    #[serde(flatten)]
    pub operation: IpcOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum IpcOperation {
    Ping,
    Reexec {
        candidate_path: PathBuf,
        candidate_version: String,
    },
    Launch {
        project_path: ProjectPath,
        #[serde(with = "crate::os_string")]
        name: OsString,
        #[serde(with = "crate::os_string::vec")]
        command: Vec<OsString>,
    },
    Ps {
        project_path: ProjectPath,
    },
    Status {
        key: ProcessKey,
    },
    Logs {
        key: ProcessKey,
        tail: Option<u64>,
        head: Option<u64>,
        follow: bool,
        grep: Option<String>,
        stdout: bool,
        stderr: bool,
    },
    Stop {
        key: ProcessKey,
        force: bool,
    },
    Signal {
        key: ProcessKey,
        signal: String,
    },
    Restart {
        key: ProcessKey,
    },
    Start {
        key: ProcessKey,
    },
    #[serde(rename = "rm")]
    Remove {
        key: ProcessKey,
        keep_logs: bool,
    },
    Clean,
    Wait {
        key: ProcessKey,
        state: Option<ProcessState>,
        match_text: Option<String>,
        exit: bool,
        timeout_ms: Option<u64>,
    },
}

impl IpcRequest {
    pub fn new(request_id: u64, operation: IpcOperation) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            request_id,
            client_version: CLIENT_VERSION.to_owned(),
            operation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpcLogOptions {
    pub tail: Option<u64>,
    pub head: Option<u64>,
    pub follow: bool,
    pub grep: Option<String>,
    pub stdout: bool,
    pub stderr: bool,
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

    let response: IpcResponse = read_message(
        stream,
        "read daemon response",
        "daemon closed the connection",
        "daemon response is too large",
    )
    .await?;
    validate_response(request, response)
}

pub async fn send_stream_request<F>(
    socket_path: &Path,
    request: &IpcRequest,
    mut on_chunk: F,
) -> Result<IpcResponse, IpcError>
where
    F: FnMut(&str),
{
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

    loop {
        let response: IpcResponse = read_message(
            &mut stream,
            "read daemon stream",
            "daemon closed the stream",
            "daemon stream message is too large",
        )
        .await?;
        let response = validate_response(request, response)?;
        if !response.result.ok {
            return Ok(response);
        }
        let data =
            response.result.data.as_ref().ok_or_else(|| {
                IpcError::Protocol("daemon stream frame is missing data".to_owned())
            })?;
        if data.get("done").and_then(serde_json::Value::as_bool) == Some(true) {
            return Ok(response);
        }
        if let Some(content) = data.get("content").and_then(serde_json::Value::as_str) {
            on_chunk(content);
        }
    }
}

fn validate_response(request: &IpcRequest, response: IpcResponse) -> Result<IpcResponse, IpcError> {
    if response.version != PROTOCOL_VERSION {
        return Err(IpcError::Protocol(format!(
            "unsupported daemon protocol version {}",
            response.version
        )));
    }
    if response.request_id != request.request_id {
        return Err(IpcError::Protocol(format!(
            "daemon response request ID {} does not match request ID {}",
            response.request_id, request.request_id
        )));
    }
    Ok(response)
}

pub async fn read_request(stream: &mut tokio::net::UnixStream) -> Result<IpcRequest, IpcError> {
    read_message(
        &mut *stream,
        "read daemon request",
        "client closed the connection",
        "daemon request is too large",
    )
    .await
}

async fn read_message<T, R>(
    mut reader: R,
    operation: &'static str,
    closed_message: &'static str,
    too_large_message: &'static str,
) -> Result<T, IpcError>
where
    T: serde::de::DeserializeOwned,
    R: AsyncRead + Unpin,
{
    let mut line = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        let bytes = reader
            .read(&mut byte)
            .await
            .map_err(|source| IpcError::Io { operation, source })?;
        if bytes == 0 {
            if line.is_empty() {
                return Err(IpcError::Protocol(closed_message.to_owned()));
            }
            return Err(IpcError::Protocol(
                "IPC message must end with a newline".to_owned(),
            ));
        }
        line.push(byte[0]);
        if line.len() > MAX_MESSAGE_BYTES {
            return Err(IpcError::Protocol(too_large_message.to_owned()));
        }
        if byte[0] == b'\n' {
            break;
        }
    }
    line.pop();
    serde_json::from_slice(&line).map_err(IpcError::Deserialize)
}

pub async fn write_response(
    stream: &mut UnixStream,
    response: &IpcResponse,
) -> Result<(), IpcError> {
    let mut payload = serde_json::to_vec(response).map_err(IpcError::Serialize)?;
    payload.push(b'\n');
    timeout(WRITE_TIMEOUT, stream.write_all(&payload))
        .await
        .map_err(|_| IpcError::Io {
            operation: "send daemon response",
            source: std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "IPC response write timed out",
            ),
        })?
        .map_err(|source| IpcError::Io {
            operation: "send daemon response",
            source,
        })
}

pub fn request_for_ps(request_id: u64, project_path: ProjectPath) -> IpcRequest {
    IpcRequest::new(request_id, IpcOperation::Ps { project_path })
}

pub fn request_for_reexec(
    request_id: u64,
    candidate_path: PathBuf,
    candidate_version: String,
) -> IpcRequest {
    IpcRequest::new(
        request_id,
        IpcOperation::Reexec {
            candidate_path,
            candidate_version,
        },
    )
}

pub fn request_for_launch(
    request_id: u64,
    project_path: ProjectPath,
    name: OsString,
    command: Vec<OsString>,
) -> IpcRequest {
    IpcRequest::new(
        request_id,
        IpcOperation::Launch {
            project_path,
            name,
            command,
        },
    )
}

pub fn request_for_status(request_id: u64, key: ProcessKey) -> IpcRequest {
    IpcRequest::new(request_id, IpcOperation::Status { key })
}

pub fn request_for_logs(request_id: u64, key: ProcessKey, options: IpcLogOptions) -> IpcRequest {
    IpcRequest::new(
        request_id,
        IpcOperation::Logs {
            key,
            tail: options.tail,
            head: options.head,
            follow: options.follow,
            grep: options.grep,
            stdout: options.stdout,
            stderr: options.stderr,
        },
    )
}

pub fn request_for_stop(request_id: u64, key: ProcessKey, force: bool) -> IpcRequest {
    IpcRequest::new(request_id, IpcOperation::Stop { key, force })
}

pub fn request_for_signal(request_id: u64, key: ProcessKey, signal: String) -> IpcRequest {
    IpcRequest::new(request_id, IpcOperation::Signal { key, signal })
}

pub fn request_for_restart(request_id: u64, key: ProcessKey) -> IpcRequest {
    IpcRequest::new(request_id, IpcOperation::Restart { key })
}

pub fn request_for_start(request_id: u64, key: ProcessKey) -> IpcRequest {
    IpcRequest::new(request_id, IpcOperation::Start { key })
}

pub fn request_for_remove(request_id: u64, key: ProcessKey, keep_logs: bool) -> IpcRequest {
    IpcRequest::new(request_id, IpcOperation::Remove { key, keep_logs })
}

pub fn request_for_clean(request_id: u64) -> IpcRequest {
    IpcRequest::new(request_id, IpcOperation::Clean)
}

pub fn request_for_wait(
    request_id: u64,
    key: ProcessKey,
    state: Option<ProcessState>,
    match_text: Option<String>,
    exit: bool,
    timeout_ms: Option<u64>,
) -> IpcRequest {
    IpcRequest::new(
        request_id,
        IpcOperation::Wait {
            key,
            state,
            match_text,
            exit,
            timeout_ms,
        },
    )
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
    use tokio::io::{AsyncWriteExt, duplex};

    #[test]
    fn serializes_versioned_status_request() {
        let project = ProjectPath::from_canonical(PathBuf::from("/project"));
        let request = request_for_status(7, ProcessKey::new(project, OsString::from("dev")));
        assert_eq!(
            serde_json::to_string(&request).expect("request should serialize"),
            format!(
                r#"{{"version":1,"request_id":7,"client_version":"{}","operation":"status","key":{{"project_path":"/project","name":"646576"}}}}"#,
                CLIENT_VERSION
            )
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

    #[test]
    fn serializes_reexec_request_fields() {
        let request =
            request_for_reexec(6, PathBuf::from("/usr/local/bin/park"), "0.3.0".to_owned());
        assert_eq!(
            serde_json::to_string(&request).expect("request should serialize"),
            format!(
                r#"{{"version":1,"request_id":6,"client_version":"{}","operation":"reexec","candidate_path":"/usr/local/bin/park","candidate_version":"0.3.0"}}"#,
                CLIENT_VERSION
            )
        );

        let decoded: IpcRequest = serde_json::from_str(
            &serde_json::to_string(&request).expect("request should serialize"),
        )
        .expect("request should deserialize");
        assert_eq!(decoded, request);
    }

    #[test]
    fn serializes_exact_launch_arguments() {
        let request = request_for_launch(
            8,
            ProjectPath::from_canonical("/project".into()),
            OsString::from("dev"),
            vec![OsString::from("server"), OsString::from("--dev")],
        );
        let decoded: IpcRequest = serde_json::from_value(
            serde_json::to_value(request).expect("request should serialize"),
        )
        .expect("request should deserialize");
        assert_eq!(decoded.request_id, 8);
        assert!(matches!(decoded.operation, IpcOperation::Launch { .. }));
    }

    #[test]
    fn serializes_wait_condition_and_timeout() {
        let request = request_for_wait(
            9,
            ProcessKey::new(
                ProjectPath::from_canonical("/project".into()),
                OsString::from("dev"),
            ),
            Some(ProcessState::Running),
            None,
            false,
            Some(500),
        );
        assert_eq!(
            serde_json::to_string(&request).expect("request should serialize"),
            format!(
                r#"{{"version":1,"request_id":9,"client_version":"{}","operation":"wait","key":{{"project_path":"/project","name":"646576"}},"state":"running","match_text":null,"exit":false,"timeout_ms":500}}"#,
                CLIENT_VERSION
            )
        );
    }

    #[test]
    fn rejects_an_oversized_unterminated_message_without_unbounded_buffering() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime should build");
        runtime.block_on(async {
            let (mut writer, reader) = duplex(MAX_MESSAGE_BYTES + 1);
            writer
                .write_all(&vec![b'x'; MAX_MESSAGE_BYTES + 1])
                .await
                .expect("test message should write");
            writer.shutdown().await.expect("writer should close");

            let error = read_message::<IpcRequest, _>(
                reader,
                "read test request",
                "test writer closed",
                "test request is too large",
            )
            .await
            .expect_err("oversized message should fail");
            assert!(matches!(error, IpcError::Protocol(message) if message == "test request is too large"));
        });
    }

    #[test]
    fn rejects_a_message_without_the_newline_delimiter() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime should build");
        runtime.block_on(async {
            let (mut writer, reader) = duplex(64);
            writer
                .write_all(br#"{"version":1}"#)
                .await
                .expect("test message should write");
            writer.shutdown().await.expect("writer should close");

            let error = read_message::<IpcRequest, _>(
                reader,
                "read test request",
                "test writer closed",
                "test request is too large",
            )
            .await
            .expect_err("unterminated message should fail");
            assert!(matches!(error, IpcError::Protocol(message) if message == "IPC message must end with a newline"));
        });
    }

    #[test]
    fn rejects_response_with_a_mismatched_version_or_request_id() {
        let request = request_for_ps(7, ProjectPath::from_canonical("/project".into()));
        let mut response = IpcResponse::success(7, None);
        response.version = PROTOCOL_VERSION + 1;
        assert!(matches!(
            validate_response(&request, response),
            Err(IpcError::Protocol(message)) if message.contains("protocol version")
        ));

        let response = IpcResponse::success(8, None);
        assert!(matches!(
            validate_response(&request, response),
            Err(IpcError::Protocol(message)) if message.contains("request ID")
        ));
    }
}
