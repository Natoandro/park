use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};
use thiserror::Error;

use crate::lifecycle::{InvalidStateTransition, ProcessState};
use crate::process::{LogPaths, ProcessKey, ProcessRecord, ProcessRecordValidationError};

mod files;
mod record_codec;
mod records;
mod schema;

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
    logs_dir: PathBuf,
    runtime_dir: PathBuf,
    database_path: PathBuf,
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
            logs_dir: state_dir.join("logs"),
            runtime_dir: runtime_base.join("park"),
            database_path: state_dir.join("park.sqlite3"),
            state_dir,
        })
    }

    pub fn from_process_environment() -> Result<Self, StorageError> {
        Self::from_environment(&XdgEnvironment::from_process())
    }

    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    pub fn logs_dir(&self) -> &Path {
        &self.logs_dir
    }

    pub fn runtime_dir(&self) -> &Path {
        &self.runtime_dir
    }

    pub fn socket_path(&self) -> PathBuf {
        self.runtime_dir.join("daemon.sock")
    }

    pub fn lock_path(&self) -> PathBuf {
        self.runtime_dir.join("daemon.lock")
    }

    pub fn pid_path(&self) -> PathBuf {
        self.runtime_dir.join("daemon.pid")
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn ensure_directories(&self) -> Result<(), StorageError> {
        for path in [self.state_dir(), self.logs_dir(), self.runtime_dir()] {
            fs::create_dir_all(path).map_err(|source| StorageError::Io {
                operation: "create directory",
                path: path.to_path_buf(),
                source,
            })?;
            files::set_private_permissions(path)?;
        }
        let mut connection =
            Connection::open(&self.database_path).map_err(|source| StorageError::Sqlite {
                operation: "open SQLite database",
                source,
            })?;
        schema::initialize(&mut connection)?;
        files::set_private_file_permissions(&self.database_path)?;
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

    pub(crate) fn remove_logs(&self, key: &ProcessKey) -> Result<(), StorageError> {
        let logs = self.log_paths(key);
        files::remove_if_present(&logs.stdout)?;
        files::remove_if_present(&logs.stderr)
    }

    pub fn remove_record(&self, key: &ProcessKey, keep_logs: bool) -> Result<(), StorageError> {
        let path = self.paths.database_path().to_path_buf();
        let record = self
            .load_record(key)?
            .ok_or_else(|| StorageError::RecordMissing { path: path.clone() })?;
        if !record.state().is_terminal() {
            return Err(StorageError::ActiveRecord { path });
        }
        let connection = self.open_connection()?;
        connection
            .execute(
                "DELETE FROM process_records WHERE key_digest = ?1",
                params![files::key_digest(key)],
            )
            .map_err(|source| StorageError::Sqlite {
                operation: "remove SQLite process record",
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
            if self.save_record_if_unchanged(
                &self
                    .load_record(&key)?
                    .ok_or_else(|| StorageError::RecordMissing {
                        path: self.paths.database_path().to_path_buf(),
                    })?,
                &record,
            )? {
                reconciled.push(key);
            }
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
    InvalidRecordInvariant {
        path: PathBuf,
        source: ProcessRecordValidationError,
    },
    #[error("invalid stored process field {field}: {value}")]
    InvalidStoredField { field: &'static str, value: String },
    #[error("unsupported SQLite schema version {version}")]
    UnsupportedSchemaVersion { version: i32 },
    #[error("could not {operation}: {source}")]
    Sqlite {
        operation: &'static str,
        source: rusqlite::Error,
    },
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
