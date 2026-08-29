use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::lifecycle::{InvalidStateTransition, ProcessState};
use crate::project::ProjectPath;

/// Unix epoch seconds used for durable lifecycle timestamps.
pub type EpochSeconds = u64;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProcessKey {
    project_path: ProjectPath,
    name: OsString,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogPaths {
    pub stdout: PathBuf,
    pub stderr: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRecord {
    key: ProcessKey,
    working_directory: PathBuf,
    executable: OsString,
    arguments: Vec<OsString>,
    pid: Option<u32>,
    process_group_id: Option<u32>,
    created_at: EpochSeconds,
    started_at: Option<EpochSeconds>,
    exited_at: Option<EpochSeconds>,
    state: ProcessState,
    exit_code: Option<i32>,
    termination_signal: Option<i32>,
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
            created_at,
            started_at: None,
            exited_at: None,
            state: ProcessState::Starting,
            exit_code: None,
            termination_signal: None,
            logs,
        }
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

    pub fn logs(&self) -> &LogPaths {
        &self.logs
    }

    pub fn transition_to(&mut self, next: ProcessState) -> Result<(), InvalidStateTransition> {
        self.state = self.state.transition_to(next)?;
        Ok(())
    }
}
