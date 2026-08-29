use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::lifecycle::{InvalidStateTransition, ProcessState};
use crate::project::ProjectPath;

/// Unix epoch seconds used for durable lifecycle timestamps.
pub type EpochSeconds = u64;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProcessKey {
    project_path: ProjectPath,
    #[serde(with = "os_string_serde")]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogPaths {
    pub stdout: PathBuf,
    pub stderr: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessRecord {
    key: ProcessKey,
    working_directory: PathBuf,
    #[serde(with = "os_string_serde")]
    executable: OsString,
    #[serde(with = "os_string_vec_serde")]
    arguments: Vec<OsString>,
    pid: Option<u32>,
    process_group_id: Option<u32>,
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

    pub fn failure_reason(&self) -> Option<&str> {
        self.failure_reason.as_deref()
    }

    pub fn logs(&self) -> &LogPaths {
        &self.logs
    }

    pub fn transition_to(&mut self, next: ProcessState) -> Result<(), InvalidStateTransition> {
        self.state = self.state.transition_to(next)?;
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

mod os_string_serde {
    use super::*;
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    pub fn serialize<S>(value: &OsString, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&encode_hex(value.as_bytes()))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<OsString, D::Error>
    where
        D: Deserializer<'de>,
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
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    pub fn serialize<S>(values: &[OsString], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encoded = values
            .iter()
            .map(|value| os_string_serde::encode_hex(value.as_bytes()))
            .collect::<Vec<_>>();
        encoded.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<OsString>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let values = Vec::<String>::deserialize(deserializer)?;
        values
            .into_iter()
            .map(|value| {
                os_string_serde::decode_hex(&value)
                    .map(OsString::from_vec)
                    .map_err(serde::de::Error::custom)
            })
            .collect()
    }
}
