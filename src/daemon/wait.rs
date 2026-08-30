use std::fs;
use std::time::Duration;

use serde_json::json;
use tokio::net::UnixStream;
use tokio::time::{Instant, sleep};

use crate::ipc::{IpcError, IpcResponse, record_value, write_response};
use crate::lifecycle::ProcessState;
use crate::process::{ProcessKey, ProcessRecord};
use crate::result::ResultStatus;

use super::{DaemonState, epoch_seconds, record_is_alive};

const POLL_INTERVAL: Duration = Duration::from_millis(50);

pub(super) async fn serve(
    state: &DaemonState,
    request_id: u64,
    key: ProcessKey,
    options: WaitOptions,
    stream: &mut UnixStream,
) -> Result<(), IpcError> {
    let WaitOptions {
        expected_state,
        match_text,
        exit,
        timeout_ms,
    } = options;
    if usize::from(expected_state.is_some()) + usize::from(match_text.is_some()) + usize::from(exit)
        != 1
    {
        return write_response(
            stream,
            &IpcResponse::error(
                request_id,
                ResultStatus::Failure,
                "wait requires exactly one condition",
            ),
        )
        .await;
    }

    let deadline =
        timeout_ms.map(|milliseconds| Instant::now() + Duration::from_millis(milliseconds));
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
        if condition_matches(&record, expected_state, match_text.as_deref(), exit) {
            let value = match record_value(&record) {
                Ok(value) => value,
                Err(error) => {
                    return write_response(
                        stream,
                        &IpcResponse::error(request_id, ResultStatus::Failure, error.to_string()),
                    )
                    .await;
                }
            };
            return write_response(
                stream,
                &IpcResponse::success(request_id, Some(json!({"done": true, "record": value}))),
            )
            .await;
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return write_response(
                stream,
                &IpcResponse::error(
                    request_id,
                    ResultStatus::Failure,
                    "timed out waiting for condition",
                ),
            )
            .await;
        }

        // Heartbeats make a disconnected client observable without holding a lifecycle lock.
        write_response(
            stream,
            &IpcResponse::success(request_id, Some(json!({"done": false}))),
        )
        .await?;
        sleep(POLL_INTERVAL).await;
    }
}

#[derive(Debug, Clone)]
pub(super) struct WaitOptions {
    pub(super) expected_state: Option<ProcessState>,
    pub(super) match_text: Option<String>,
    pub(super) exit: bool,
    pub(super) timeout_ms: Option<u64>,
}

fn condition_matches(
    record: &ProcessRecord,
    expected_state: Option<ProcessState>,
    match_text: Option<&str>,
    exit: bool,
) -> bool {
    if let Some(expected_state) = expected_state {
        return record.state() == expected_state;
    }
    if exit {
        return record.state().is_terminal();
    }
    let Some(match_text) = match_text else {
        return false;
    };
    if match_text.is_empty() {
        return true;
    }
    let pattern = match_text.as_bytes();
    [
        record.logs().stdout.as_path(),
        record.logs().stderr.as_path(),
    ]
    .into_iter()
    .filter_map(|path| fs::read(path).ok())
    .any(|content| {
        content
            .windows(pattern.len())
            .any(|window| window == pattern)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::PathBuf;

    use crate::{LogPaths, ProjectPath};

    fn record(state: ProcessState, stdout: &str, stderr: &str) -> ProcessRecord {
        let mut record = ProcessRecord::new(
            ProcessKey::new(
                ProjectPath::from_canonical("/project".into()),
                OsString::from("dev"),
            ),
            PathBuf::from("/project"),
            OsString::from("server"),
            vec![],
            1,
            LogPaths {
                stdout: PathBuf::from(stdout),
                stderr: PathBuf::from(stderr),
            },
        );
        if state == ProcessState::Running {
            record
                .mark_running(2, 123, Some(123), Some(123))
                .expect("record should become running");
        } else if state == ProcessState::Failed {
            record
                .mark_spawn_failed(2, "test failure")
                .expect("record should become failed");
        }
        record
    }

    #[test]
    fn matches_states_and_terminal_exit() {
        let record = record(ProcessState::Failed, "/missing", "/missing");
        assert!(condition_matches(
            &record,
            Some(ProcessState::Failed),
            None,
            false
        ));
        assert!(condition_matches(&record, None, None, true));
        assert!(!condition_matches(
            &record,
            Some(ProcessState::Running),
            None,
            false
        ));
    }

    #[test]
    fn empty_match_is_immediate() {
        let record = record(ProcessState::Running, "/missing", "/missing");
        assert!(condition_matches(&record, None, Some(""), false));
    }
}
