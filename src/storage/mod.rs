use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Error as JsonError;
use thiserror::Error;

use crate::lifecycle::{InvalidStateTransition, ProcessState};
use crate::process::{LogPaths, ProcessKey, ProcessRecord};

mod files;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XdgEnvironment {
    pub state_home: Option<PathBuf>,
    pub runtime_dir: Option<PathBuf>,
    pub home: Option<PathBuf>,
}

impl XdgEnvironment {
    pub fn from_process() -> Self {
        Self {
            state_home: env::var_os("XDG_STATE_HOME").map(PathBuf::from),
            runtime_dir: env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from),
            home: env::var_os("HOME").map(PathBuf::from),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoragePaths {
    state_dir: PathBuf,
    records_dir: PathBuf,
    logs_dir: PathBuf,
    runtime_dir: PathBuf,
}

impl StoragePaths {
    pub fn from_environment(environment: &XdgEnvironment) -> Result<Self, StorageError> {
        let state_home = match environment.state_home.as_deref() {
            Some(path) if !path.as_os_str().is_empty() => path.to_path_buf(),
            _ => environment
                .home
                .as_deref()
                .filter(|path| !path.as_os_str().is_empty())
                .map(|home| home.join(".local").join("state"))
                .ok_or(StorageError::MissingHome)?,
        };
        let state_dir = state_home.join("park");
        let runtime_base = environment
            .runtime_dir
            .as_deref()
            .filter(|path| !path.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| state_dir.join("runtime"));

        Ok(Self {
            records_dir: state_dir.join("records"),
            logs_dir: state_dir.join("logs"),
            runtime_dir: runtime_base.join("park"),
            state_dir,
        })
    }

    pub fn from_process_environment() -> Result<Self, StorageError> {
        Self::from_environment(&XdgEnvironment::from_process())
    }

    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    pub fn records_dir(&self) -> &Path {
        &self.records_dir
    }

    pub fn logs_dir(&self) -> &Path {
        &self.logs_dir
    }

    pub fn runtime_dir(&self) -> &Path {
        &self.runtime_dir
    }

    pub fn ensure_directories(&self) -> Result<(), StorageError> {
        for path in [
            self.state_dir(),
            self.records_dir(),
            self.logs_dir(),
            self.runtime_dir(),
        ] {
            fs::create_dir_all(path).map_err(|source| StorageError::Io {
                operation: "create directory",
                path: path.to_path_buf(),
                source,
            })?;
            files::set_private_permissions(path)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Storage {
    paths: StoragePaths,
}

impl Storage {
    pub fn new(paths: StoragePaths) -> Self {
        Self { paths }
    }

    pub fn paths(&self) -> &StoragePaths {
        &self.paths
    }

    pub fn record_path(&self, key: &ProcessKey) -> PathBuf {
        self.paths.records_dir().join(files::key_record_name(key))
    }

    pub fn log_paths(&self, key: &ProcessKey) -> LogPaths {
        let prefix = files::key_log_prefix(key);
        LogPaths {
            stdout: self.paths.logs_dir().join(format!("{prefix}.stdout.log")),
            stderr: self.paths.logs_dir().join(format!("{prefix}.stderr.log")),
        }
    }

    pub fn create_logs(&self, key: &ProcessKey) -> Result<LogPaths, StorageError> {
        self.paths.ensure_directories()?;
        let paths = self.log_paths(key);
        let stdout = files::create_new_file(&paths.stdout)?;
        if let Err(error) = files::create_new_file(&paths.stderr) {
            drop(stdout);
            let _ = fs::remove_file(&paths.stdout);
            return Err(error);
        }
        Ok(paths)
    }

    pub fn create_record(&self, record: &ProcessRecord) -> Result<(), StorageError> {
        self.paths.ensure_directories()?;
        let path = self.record_path(record.key());
        if path.exists() {
            return Err(StorageError::RecordExists { path });
        }
        let payload = serde_json::to_vec_pretty(record)?;
        files::atomic_create(&path, &payload)
    }

    pub fn save_record(&self, record: &ProcessRecord) -> Result<(), StorageError> {
        self.paths.ensure_directories()?;
        let path = self.record_path(record.key());
        if !path.exists() {
            return Err(StorageError::RecordMissing { path });
        }
        let payload = serde_json::to_vec_pretty(record)?;
        files::atomic_replace(&path, &payload)
    }

    pub fn load_record(&self, key: &ProcessKey) -> Result<Option<ProcessRecord>, StorageError> {
        let path = self.record_path(key);
        if !path.exists() {
            return Ok(None);
        }
        files::read_record(&path).map(Some)
    }

    pub fn list_records(&self) -> Result<Vec<ProcessRecord>, StorageError> {
        let entries = match fs::read_dir(self.paths.records_dir()) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => {
                return Err(StorageError::Io {
                    operation: "read records directory",
                    path: self.paths.records_dir().to_path_buf(),
                    source,
                });
            }
        };
        files::read_records(entries)
    }

    pub fn remove_record(&self, key: &ProcessKey, keep_logs: bool) -> Result<(), StorageError> {
        let path = self.record_path(key);
        let record = self
            .load_record(key)?
            .ok_or_else(|| StorageError::RecordMissing { path: path.clone() })?;
        if !record.state().is_terminal() {
            return Err(StorageError::ActiveRecord { path });
        }
        fs::remove_file(&path).map_err(|source| StorageError::Io {
            operation: "remove record",
            path: path.clone(),
            source,
        })?;
        if !keep_logs {
            files::remove_if_present(&record.logs().stdout)?;
            files::remove_if_present(&record.logs().stderr)?;
        }
        Ok(())
    }

    pub fn reconcile<F>(
        &self,
        exited_at: u64,
        mut is_alive: F,
    ) -> Result<Vec<ProcessKey>, StorageError>
    where
        F: FnMut(&ProcessRecord) -> bool,
    {
        let mut reconciled = Vec::new();
        for mut record in self.list_records()? {
            if record.state().is_terminal() || is_alive(&record) {
                continue;
            }
            match record.state() {
                ProcessState::Starting => record.reconcile_as_failed(
                    exited_at,
                    "process was not running during startup reconciliation",
                )?,
                ProcessState::Running | ProcessState::Stopping => {
                    record.reconcile_as_exited(exited_at)?
                }
                ProcessState::Exited | ProcessState::Failed | ProcessState::Killed => continue,
            }
            let key = record.key().clone();
            self.save_record(&record)?;
            reconciled.push(key);
        }
        Ok(reconciled)
    }
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("HOME is required when XDG_STATE_HOME is not set")]
    MissingHome,
    #[error("a record already exists at {path:?}")]
    RecordExists { path: PathBuf },
    #[error("no record exists at {path:?}")]
    RecordMissing { path: PathBuf },
    #[error("cannot remove active record at {path:?}")]
    ActiveRecord { path: PathBuf },
    #[error("invalid record at {path:?}: {source}")]
    InvalidRecord { path: PathBuf, source: JsonError },
    #[error("could not serialize record: {0}")]
    Json(#[from] JsonError),
    #[error("could not reconcile record state: {0}")]
    StateTransition(#[from] InvalidStateTransition),
    #[error("could not {operation} {path:?}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

#[cfg(test)]
mod tests;
