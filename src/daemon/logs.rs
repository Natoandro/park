use std::fs;

use serde_json::json;
use tokio::net::UnixStream;
use tokio::time::{Duration, sleep};

use crate::ipc::{IpcError, IpcResponse, write_response};
use crate::process::ProcessKey;
use crate::result::ResultStatus;

use super::{DaemonState, epoch_seconds, record_is_alive};

const FOLLOW_INTERVAL: Duration = Duration::from_millis(50);
const FRAME_BYTES: usize = 16 * 1024;

pub(super) async fn serve(
    state: &DaemonState,
    request_id: u64,
    key: ProcessKey,
    options: LogOptions,
    stream: &mut UnixStream,
) -> Result<(), IpcError> {
    let mut stdout_offset = 0;
    let mut stderr_offset = 0;
    let mut rendered_offset = 0;
    let mut initial = true;

    loop {
        if let Err(error) = state.storage.reconcile(epoch_seconds(), record_is_alive) {
            return write_response(
                stream,
                &IpcResponse::error(request_id, ResultStatus::Failure, error.to_string()),
            )
            .await;
        }
        let record = match state.storage.load_record(&key) {
            Ok(Some(record)) => record,
            Ok(None) => {
                return write_response(
                    stream,
                    &IpcResponse::error(
                        request_id,
                        ResultStatus::MissingRecord,
                        "no process record exists",
                    ),
                )
                .await;
            }
            Err(error) => {
                return write_response(
                    stream,
                    &IpcResponse::error(request_id, ResultStatus::Failure, error.to_string()),
                )
                .await;
            }
        };
        let logs = record.logs();
        let stdout = match read_log(&logs.stdout) {
            Ok(stdout) => stdout,
            Err(error) => return send_log_error(stream, request_id, error).await,
        };
        let stderr = match read_log(&logs.stderr) {
            Ok(stderr) => stderr,
            Err(error) => return send_log_error(stream, request_id, error).await,
        };
        let all = selected_output(&stdout, &stderr, &options);
        let content = if initial {
            render_output(&all, &options)
        } else if options.grep.is_some() {
            suffix_after_rendered(&all, &options, rendered_offset)
        } else {
            selected_delta(&stdout, &stderr, &options, stdout_offset, stderr_offset)
        };
        send_content(stream, request_id, stream_name(&options), &content).await?;
        stdout_offset = stdout.len();
        stderr_offset = stderr.len();
        rendered_offset = render_output(&all, &options).len();
        initial = false;

        if !options.follow || record.state().is_terminal() {
            let done = IpcResponse::success(
                request_id,
                Some(json!({
                    "done": true,
                    "stream": stream_name(&options),
                    "state": record.state(),
                })),
            );
            return write_response(stream, &done).await;
        }
        sleep(FOLLOW_INTERVAL).await;
    }
}

#[derive(Debug, Clone)]
pub(super) struct LogOptions {
    pub(super) tail: Option<u64>,
    pub(super) head: Option<u64>,
    pub(super) follow: bool,
    pub(super) grep: Option<String>,
    pub(super) stdout: bool,
    pub(super) stderr: bool,
}

fn read_log(path: &std::path::Path) -> Result<Vec<u8>, IpcError> {
    fs::read(path).map_err(|source| IpcError::Io {
        operation: "read process log",
        source,
    })
}

async fn send_log_error(
    stream: &mut UnixStream,
    request_id: u64,
    error: IpcError,
) -> Result<(), IpcError> {
    write_response(
        stream,
        &IpcResponse::error(request_id, ResultStatus::Failure, error.to_string()),
    )
    .await
}

fn selected_output(stdout: &[u8], stderr: &[u8], options: &LogOptions) -> Vec<u8> {
    if options.stdout {
        stdout.to_vec()
    } else if options.stderr {
        stderr.to_vec()
    } else {
        let mut combined = stdout.to_vec();
        combined.extend_from_slice(stderr);
        combined
    }
}

fn selected_delta(
    stdout: &[u8],
    stderr: &[u8],
    options: &LogOptions,
    stdout_offset: usize,
    stderr_offset: usize,
) -> Vec<u8> {
    if options.stdout {
        stdout.get(stdout_offset..).unwrap_or_default().to_vec()
    } else if options.stderr {
        stderr.get(stderr_offset..).unwrap_or_default().to_vec()
    } else {
        let mut combined = stdout.get(stdout_offset..).unwrap_or_default().to_vec();
        combined.extend_from_slice(stderr.get(stderr_offset..).unwrap_or_default());
        combined
    }
}

fn render_output(bytes: &[u8], options: &LogOptions) -> Vec<u8> {
    let mut lines = bytes
        .split_inclusive(|byte| *byte == b'\n')
        .filter(|line| {
            options.grep.as_ref().is_none_or(|pattern| {
                let pattern = pattern.as_bytes();
                pattern.is_empty() || line.windows(pattern.len()).any(|part| part == pattern)
            })
        })
        .collect::<Vec<_>>();
    if let Some(head) = options.head {
        lines.truncate(head as usize);
    } else if let Some(tail) = options.tail {
        let keep = tail as usize;
        let start = lines.len().saturating_sub(keep);
        lines.drain(..start);
    }
    lines.into_iter().flatten().copied().collect()
}

fn suffix_after_rendered(bytes: &[u8], options: &LogOptions, previous_size: usize) -> Vec<u8> {
    let rendered = render_output(
        bytes,
        &LogOptions {
            tail: None,
            head: None,
            follow: options.follow,
            grep: options.grep.clone(),
            stdout: options.stdout,
            stderr: options.stderr,
        },
    );
    rendered.get(previous_size..).unwrap_or_default().to_vec()
}

async fn send_content(
    stream: &mut UnixStream,
    request_id: u64,
    stream_name: &str,
    content: &[u8],
) -> Result<(), IpcError> {
    let text = String::from_utf8_lossy(content);
    if text.is_empty() {
        return Ok(());
    }
    let mut offset = 0;
    while offset < text.len() {
        let mut end = (offset + FRAME_BYTES).min(text.len());
        while end > offset && !text.is_char_boundary(end) {
            end -= 1;
        }
        let frame = IpcResponse::success(
            request_id,
            Some(json!({
                "done": false,
                "stream": stream_name,
                "content": &text[offset..end],
            })),
        );
        write_response(stream, &frame).await?;
        offset = end;
    }
    Ok(())
}

fn stream_name(options: &LogOptions) -> &'static str {
    if options.stdout {
        "stdout"
    } else if options.stderr {
        "stderr"
    } else {
        "combined"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> LogOptions {
        LogOptions {
            tail: None,
            head: None,
            follow: false,
            grep: None,
            stdout: false,
            stderr: false,
        }
    }

    #[test]
    fn combined_output_is_stdout_then_stderr() {
        let options = options();
        assert_eq!(selected_output(b"out", b"err", &options), b"outerr");
    }

    #[test]
    fn filters_lines_before_applying_tail() {
        let mut options = options();
        options.grep = Some("keep".to_owned());
        options.tail = Some(1);
        assert_eq!(
            render_output(b"keep one\ndrop\nkeep two\n", &options),
            b"keep two\n"
        );
    }
}
