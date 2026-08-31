use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::lifecycle::{InvalidStateTransition, ProcessState};
use crate::project::ProjectPath;

/// Unix epoch seconds used for durable lifecycle timestamps.
pub type EpochSeconds = u64;

pub fn validate_process_name(name: &OsStr) -> Result<(), ProcessNameError> {
    let bytes = name.as_bytes();
    if bytes.is_empty() {
        return Err(ProcessNameError::Empty);
    }
    if let Some((index, byte)) = bytes.iter().copied().enumerate().find(|(_, byte)| {
        !byte.is_ascii_alphanumeric() && !matches!(byte, b'.' | b'_' | b'-' | b':')
    }) {
        return Err(ProcessNameError::InvalidByte { index, byte });
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProcessNameError {
    #[error("process name must not be empty")]
    Empty,
    #[error(
        "process name contains unsupported byte 0x{byte:02x} at byte {index}; use ASCII letters, digits, '.', '_', '-' or ':'"
    )]
    InvalidByte { index: usize, byte: u8 },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProcessKey {
    project_path: ProjectPath,
    #[serde(with = "crate::os_string")]
    name: OsString,
}

pub(crate) struct ProcessRecordParts {
    pub key: ProcessKey,
    pub executable: OsString,
    pub arguments: Vec<OsString>,
    pub pid: Option<u32>,
    pub process_group_id: Option<u32>,
    pub process_start_time: Option<u64>,
    pub created_at: EpochSeconds,
    pub started_at: Option<EpochSeconds>,
    pub exited_at: Option<EpochSeconds>,
    pub state: ProcessState,
    pub exit_code: Option<i32>,
    pub termination_signal: Option<i32>,
    pub failure_reason: Option<String>,
    pub logs: LogPaths,
}

impl ProcessKey {
    pub fn new(project_path: ProjectPath, name: OsString) -> Self {
        Self { project_path, name }
    }

    pub fn project_path(&self) -> &Path {
        self.project_path.as_path()
    }

    pub fn name(&self) -> &OsStr {
        &self.name
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogPaths {
    pub stdout: PathBuf,
    pub stderr: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessRecord {
    key: ProcessKey,
    working_directory: PathBuf,
    #[serde(with = "crate::os_string")]
    executable: OsString,
    #[serde(with = "crate::os_string::vec")]
    arguments: Vec<OsString>,
    pid: Option<u32>,
    process_group_id: Option<u32>,
    #[serde(default)]
    process_start_time: Option<u64>,
    created_at: EpochSeconds,
    started_at: Option<EpochSeconds>,
    exited_at: Option<EpochSeconds>,
    state: ProcessState,
    exit_code: Option<i32>,
    termination_signal: Option<i32>,
    failure_reason: Option<String>,
    logs: LogPaths,
}

impl ProcessRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        key: ProcessKey,
        working_directory: PathBuf,
        executable: OsString,
        arguments: Vec<OsString>,
        created_at: EpochSeconds,
        logs: LogPaths,
    ) -> Self {
        Self {
            key,
            working_directory,
            executable,
            arguments,
            pid: None,
            process_group_id: None,
            process_start_time: None,
            created_at,
            started_at: None,
            exited_at: None,
            state: ProcessState::Starting,
            exit_code: None,
            termination_signal: None,
            failure_reason: None,
            logs,
        }
    }

    pub(crate) fn from_storage(
        parts: ProcessRecordParts,
    ) -> Result<Self, ProcessRecordValidationError> {
        let ProcessRecordParts {
            key,
            executable,
            arguments,
            pid,
            process_group_id,
            process_start_time,
            created_at,
            started_at,
            exited_at,
            state,
            exit_code,
            termination_signal,
            failure_reason,
            logs,
        } = parts;
        let record = Self {
            working_directory: key.project_path().to_path_buf(),
            key,
            executable,
            arguments,
            pid,
            process_group_id,
            process_start_time,
            created_at,
            started_at,
            exited_at,
            state,
            exit_code,
            termination_signal,
            failure_reason,
            logs,
        };
        record.validate()?;
        Ok(record)
    }

    pub fn key(&self) -> &ProcessKey {
        &self.key
    }

    pub fn working_directory(&self) -> &Path {
        &self.working_directory
    }

    pub fn executable(&self) -> &OsStr {
        &self.executable
    }

    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    pub const fn pid(&self) -> Option<u32> {
        self.pid
    }

    pub const fn process_group_id(&self) -> Option<u32> {
        self.process_group_id
    }

    pub const fn process_start_time(&self) -> Option<u64> {
        self.process_start_time
    }

    pub const fn created_at(&self) -> EpochSeconds {
        self.created_at
    }

    pub const fn started_at(&self) -> Option<EpochSeconds> {
        self.started_at
    }

    pub const fn exited_at(&self) -> Option<EpochSeconds> {
        self.exited_at
    }

    pub const fn state(&self) -> ProcessState {
        self.state
    }

    pub const fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    pub const fn termination_signal(&self) -> Option<i32> {
        self.termination_signal
    }

    pub fn failure_reason(&self) -> Option<&str> {
        self.failure_reason.as_deref()
    }

    pub fn logs(&self) -> &LogPaths {
        &self.logs
    }

    pub(crate) fn validate(&self) -> Result<(), ProcessRecordValidationError> {
        if !self.key.project_path().is_absolute() {
            return Err(ProcessRecordValidationError::ProjectPath);
        }
        if self.working_directory != self.key.project_path() {
            return Err(ProcessRecordValidationError::WorkingDirectory);
        }
        if self.executable.is_empty() {
            return Err(ProcessRecordValidationError::Executable);
        }
        if !timestamps_are_ordered(self.created_at, self.started_at, self.exited_at) {
            return Err(ProcessRecordValidationError::Timestamps);
        }
        if !identifiers_are_valid(self.pid, self.process_group_id, self.process_start_time) {
            return Err(ProcessRecordValidationError::Identifiers);
        }
        let active = matches!(self.state, ProcessState::Running | ProcessState::Stopping);
        let terminal = self.state.is_terminal();
        if active && !active_identity_is_complete(self) {
            return Err(ProcessRecordValidationError::ActiveFields);
        }
        if self.state == ProcessState::Starting
            && (self.pid.is_some()
                || self.process_group_id.is_some()
                || self.process_start_time.is_some()
                || self.started_at.is_some()
                || self.exited_at.is_some()
                || self.exit_code.is_some()
                || self.termination_signal.is_some()
                || self.failure_reason.is_some())
        {
            return Err(ProcessRecordValidationError::StartingFields);
        }
        if terminal && self.exited_at.is_none() {
            return Err(ProcessRecordValidationError::TerminalTimestamp);
        }
        if self.state == ProcessState::Failed && self.failure_reason.is_none() {
            return Err(ProcessRecordValidationError::FailureReason);
        }
        if self.state == ProcessState::Killed && self.termination_signal.is_none() {
            return Err(ProcessRecordValidationError::TerminationSignal);
        }
        if self.state != ProcessState::Killed && self.termination_signal.is_some() {
            return Err(ProcessRecordValidationError::UnexpectedTerminationSignal);
        }
        Ok(())
    }

    pub fn transition_to(&mut self, next: ProcessState) -> Result<(), InvalidStateTransition> {
        self.state = self.state.transition_to(next)?;
        Ok(())
    }

    pub fn mark_running(
        &mut self,
        started_at: EpochSeconds,
        pid: u32,
        process_group_id: Option<u32>,
        process_start_time: Option<u64>,
    ) -> Result<(), InvalidStateTransition> {
        self.transition_to(ProcessState::Running)?;
        self.pid = Some(pid);
        self.process_group_id = process_group_id;
        self.process_start_time = process_start_time;
        self.started_at = Some(started_at);
        Ok(())
    }

    pub fn reset_for_start(&mut self) -> Result<(), crate::lifecycle::InvalidLifecycleAction> {
        self.state
            .validate_action(crate::lifecycle::LifecycleAction::Start)?;
        self.pid = None;
        self.process_group_id = None;
        self.process_start_time = None;
        self.started_at = None;
        self.exited_at = None;
        self.state = ProcessState::Starting;
        self.exit_code = None;
        self.termination_signal = None;
        self.failure_reason = None;
        Ok(())
    }

    pub fn mark_spawn_failed(
        &mut self,
        exited_at: EpochSeconds,
        reason: impl Into<String>,
    ) -> Result<(), InvalidStateTransition> {
        self.transition_to(ProcessState::Failed)?;
        self.exited_at = Some(exited_at);
        self.failure_reason = Some(reason.into());
        Ok(())
    }

    pub fn mark_monitor_failed(
        &mut self,
        exited_at: EpochSeconds,
        reason: impl Into<String>,
    ) -> Result<(), InvalidStateTransition> {
        self.transition_to(ProcessState::Failed)?;
        self.exited_at = Some(exited_at);
        self.failure_reason = Some(reason.into());
        Ok(())
    }

    pub fn mark_terminated(
        &mut self,
        exited_at: EpochSeconds,
        exit_code: Option<i32>,
        termination_signal: Option<i32>,
    ) -> Result<(), InvalidStateTransition> {
        let state = if termination_signal.is_some() {
            ProcessState::Killed
        } else {
            ProcessState::Exited
        };
        self.transition_to(state)?;
        self.exited_at = Some(exited_at);
        self.exit_code = exit_code;
        self.termination_signal = termination_signal;
        Ok(())
    }

    pub fn reconcile_as_exited(
        &mut self,
        exited_at: EpochSeconds,
    ) -> Result<(), InvalidStateTransition> {
        self.transition_to(ProcessState::Exited)?;
        self.exited_at = Some(exited_at);
        self.failure_reason =
            Some("process was not running during startup reconciliation".to_owned());
        Ok(())
    }

    pub fn reconcile_as_failed(
        &mut self,
        exited_at: EpochSeconds,
        reason: impl Into<String>,
    ) -> Result<(), InvalidStateTransition> {
        self.transition_to(ProcessState::Failed)?;
        self.exited_at = Some(exited_at);
        self.failure_reason = Some(reason.into());
        Ok(())
    }
}

fn timestamps_are_ordered(
    created_at: EpochSeconds,
    started_at: Option<EpochSeconds>,
    exited_at: Option<EpochSeconds>,
) -> bool {
    started_at.is_none_or(|started_at| started_at >= created_at)
        && exited_at.is_none_or(|exited_at| exited_at >= created_at)
        && !matches!((started_at, exited_at), (Some(started_at), Some(exited_at)) if exited_at < started_at)
}

fn identifiers_are_valid(
    pid: Option<u32>,
    process_group_id: Option<u32>,
    process_start_time: Option<u64>,
) -> bool {
    let valid_id = |id: u32| i32::try_from(id).is_ok_and(|id| id > 0);
    pid.is_none_or(valid_id)
        && process_group_id.is_none_or(valid_id)
        && process_start_time.is_none_or(|start_time| start_time > 0)
        && (pid.is_some() == process_group_id.is_some())
}

fn active_identity_is_complete(record: &ProcessRecord) -> bool {
    if record.pid.is_none() || record.process_group_id.is_none() || record.started_at.is_none() {
        return false;
    }
    #[cfg(target_os = "linux")]
    if record.process_start_time.is_none() {
        return false;
    }
    true
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ProcessRecordValidationError {
    #[error("record file does not match the process key")]
    RecordPath,
    #[error("log paths do not match the process key")]
    LogPaths,
    #[error("project key is not an absolute path")]
    ProjectPath,
    #[error("working directory does not match the project key")]
    WorkingDirectory,
    #[error("executable is empty")]
    Executable,
    #[error("record timestamps are out of order")]
    Timestamps,
    #[error("record process identifiers are invalid")]
    Identifiers,
    #[error("active record is missing process identifiers or start time")]
    ActiveFields,
    #[error("starting record contains lifecycle fields")]
    StartingFields,
    #[error("terminal record is missing its exit timestamp")]
    TerminalTimestamp,
    #[error("failed record is missing its failure reason")]
    FailureReason,
    #[error("killed record is missing its termination signal")]
    TerminationSignal,
    #[error("non-killed record has a termination signal")]
    UnexpectedTerminationSignal,
}
