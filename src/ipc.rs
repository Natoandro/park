use std::ffi::OsString;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader};
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
    Launch {
        project_path: ProjectPath,
        #[serde(with = "os_string_serde")]
        name: OsString,
        #[serde(with = "os_string_vec_serde")]
        command: Vec<OsString>,
    },
    Ps {
        project_path: ProjectPath,
    },
    Status {
        key: ProcessKey,
    },
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
    reader: R,
    operation: &'static str,
    closed_message: &'static str,
    too_large_message: &'static str,
) -> Result<T, IpcError>
where
    T: serde::de::DeserializeOwned,
    R: AsyncRead + Unpin,
{
    let mut line = Vec::new();
    let mut reader = BufReader::new(reader).take((MAX_MESSAGE_BYTES + 1) as u64);
    let bytes = reader
        .read_until(b'\n', &mut line)
        .await
        .map_err(|source| IpcError::Io { operation, source })?;
    if bytes == 0 {
        return Err(IpcError::Protocol(closed_message.to_owned()));
    }
    if line.len() > MAX_MESSAGE_BYTES {
        return Err(IpcError::Protocol(too_large_message.to_owned()));
    }
    if line.pop() != Some(b'\n') {
        return Err(IpcError::Protocol(
            "IPC message must end with a newline".to_owned(),
        ));
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

pub fn request_for_launch(
    request_id: u64,
    project_path: ProjectPath,
    name: OsString,
    command: Vec<OsString>,
) -> IpcRequest {
    IpcRequest {
        version: PROTOCOL_VERSION,
        request_id,
        operation: IpcOperation::Launch {
            project_path,
            name,
            command,
        },
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

mod os_string_serde {
    use super::*;
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    pub fn serialize<S>(value: &OsString, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&encode_hex(value.as_bytes()))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<OsString, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        decode_hex(&value)
            .map(OsString::from_vec)
            .map_err(serde::de::Error::custom)
    }

    pub(super) fn encode_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    pub(super) fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
        if value.len() % 2 != 0 {
            return Err("encoded OS string has odd length".to_owned());
        }
        (0..value.len())
            .step_by(2)
            .map(|index| {
                u8::from_str_radix(&value[index..index + 2], 16)
                    .map_err(|_| "invalid hexadecimal OS string".to_owned())
            })
            .collect()
    }
}

mod os_string_vec_serde {
    use super::*;

    pub fn serialize<S>(values: &[OsString], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        values
            .iter()
            .map(|value| {
                use std::os::unix::ffi::OsStrExt;
                os_string_serde::encode_hex(value.as_bytes())
            })
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<OsString>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let values = Vec::<String>::deserialize(deserializer)?;
        values
            .into_iter()
            .map(|value| {
                use std::os::unix::ffi::OsStringExt;
                os_string_serde::decode_hex(&value)
                    .map(OsString::from_vec)
                    .map_err(serde::de::Error::custom)
            })
            .collect()
    }
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
