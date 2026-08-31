use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use thiserror::Error;

use crate::lifecycle::{InvalidStateTransition, ProcessState};
use crate::process::{
    LogPaths, ProcessKey, ProcessRecord, ProcessRecordParts, ProcessRecordValidationError,
};
use crate::project::ProjectPath;

mod files;

const SCHEMA_VERSION: i32 = 1;

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
        initialize_schema(&mut connection)?;
        files::set_private_file_permissions(&self.database_path)?;
        Ok(())
    }
}

fn initialize_schema(connection: &mut Connection) -> Result<(), StorageError> {
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|source| StorageError::Sqlite {
            operation: "enable SQLite foreign keys",
            source,
        })?;
    let version = connection
        .query_row("PRAGMA user_version", [], |row| row.get::<_, i32>(0))
        .map_err(|source| StorageError::Sqlite {
            operation: "read SQLite schema version",
            source,
        })?;
    if version > SCHEMA_VERSION {
        return Err(StorageError::UnsupportedSchemaVersion { version });
    }

    create_schema(connection)?;
    if version != SCHEMA_VERSION {
        connection
            .execute_batch("PRAGMA user_version = 1;")
            .map_err(|source| StorageError::Sqlite {
                operation: "set SQLite schema version",
                source,
            })?;
    }
    Ok(())
}

fn create_schema(connection: &Connection) -> Result<(), StorageError> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS process_records (
                key_digest TEXT PRIMARY KEY NOT NULL,
                project_path BLOB NOT NULL,
                name BLOB NOT NULL,
                executable BLOB NOT NULL,
                pid INTEGER,
                process_group_id INTEGER,
                process_start_time TEXT,
                created_at TEXT NOT NULL,
                started_at TEXT,
                exited_at TEXT,
                state TEXT NOT NULL,
                exit_code INTEGER,
                termination_signal INTEGER,
                failure_reason TEXT
            );
            CREATE UNIQUE INDEX IF NOT EXISTS process_records_identity
                ON process_records(project_path, name);
            CREATE TABLE IF NOT EXISTS process_arguments (
                key_digest TEXT NOT NULL,
                position INTEGER NOT NULL,
                value BLOB NOT NULL,
                PRIMARY KEY(key_digest, position),
                FOREIGN KEY(key_digest) REFERENCES process_records(key_digest)
                    ON DELETE CASCADE
            );",
        )
        .map_err(|source| StorageError::Sqlite {
            operation: "initialize SQLite schema",
            source,
        })
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

    pub fn create_record(&self, record: &ProcessRecord) -> Result<(), StorageError> {
        self.paths.ensure_directories()?;
        let path = self.paths.database_path().to_path_buf();
        self.validate_record(record)?;
        let mut connection = self.open_connection()?;
        let transaction = connection
            .transaction()
            .map_err(|source| StorageError::Sqlite {
                operation: "start SQLite record creation",
                source,
            })?;
        insert_record(&transaction, record).map_err(|source| match source {
            rusqlite::Error::SqliteFailure(error, _)
                if error.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                StorageError::RecordExists { path }
            }
            source => StorageError::Sqlite {
                operation: "create SQLite process record",
                source,
            },
        })?;
        transaction.commit().map_err(|source| StorageError::Sqlite {
            operation: "commit SQLite process record",
            source,
        })
    }

    pub fn save_record(&self, record: &ProcessRecord) -> Result<(), StorageError> {
        self.paths.ensure_directories()?;
        let path = self.paths.database_path().to_path_buf();
        self.validate_record(record)?;
        let mut connection = self.open_connection()?;
        let transaction = connection
            .transaction()
            .map_err(|source| StorageError::Sqlite {
                operation: "start SQLite record update",
                source,
            })?;
        let changed =
            update_record(&transaction, record).map_err(|source| StorageError::Sqlite {
                operation: "save SQLite process record",
                source,
            })?;
        if changed == 0 {
            return Err(StorageError::RecordMissing { path });
        }
        transaction.commit().map_err(|source| StorageError::Sqlite {
            operation: "commit SQLite process record",
            source,
        })
    }

    pub(crate) fn save_record_if_unchanged(
        &self,
        expected: &ProcessRecord,
        updated: &ProcessRecord,
    ) -> Result<bool, StorageError> {
        self.paths.ensure_directories()?;
        self.validate_record(updated)?;
        let mut connection = self.open_connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| StorageError::Sqlite {
                operation: "start conditional SQLite record update",
                source,
            })?;
        let current = load_record_with_connection(self, &transaction, expected.key())?;
        if current.as_ref() != Some(expected) {
            return Ok(false);
        }
        update_record(&transaction, updated).map_err(|source| StorageError::Sqlite {
            operation: "conditionally save SQLite process record",
            source,
        })?;
        transaction
            .commit()
            .map_err(|source| StorageError::Sqlite {
                operation: "commit conditional SQLite process record",
                source,
            })?;
        Ok(true)
    }

    pub fn load_record(&self, key: &ProcessKey) -> Result<Option<ProcessRecord>, StorageError> {
        let connection = self.open_connection()?;
        load_record_with_connection(self, &connection, key)
    }

    pub fn list_records(&self) -> Result<Vec<ProcessRecord>, StorageError> {
        let connection = self.open_connection()?;
        let mut statement = connection
            .prepare(
                "SELECT key_digest, project_path, name, executable, pid,
                        process_group_id, process_start_time, created_at, started_at,
                        exited_at, state, exit_code, termination_signal, failure_reason
                 FROM process_records ORDER BY project_path, name",
            )
            .map_err(|source| StorageError::Sqlite {
                operation: "query SQLite process records",
                source,
            })?;
        let rows = statement
            .query_map([], StoredRecord::from_row)
            .map_err(|source| StorageError::Sqlite {
                operation: "read SQLite process records",
                source,
            })?;
        let stored_records =
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|source| StorageError::Sqlite {
                    operation: "read SQLite process record row",
                    source,
                })?;
        drop(statement);
        stored_records
            .into_iter()
            .map(|stored| {
                let arguments = load_arguments(&connection, &stored.digest)?;
                decode_stored_record(self, stored, arguments, None)
            })
            .collect()
    }

    fn validate_record(&self, record: &ProcessRecord) -> Result<(), StorageError> {
        record
            .validate()
            .map_err(|source| StorageError::InvalidRecordInvariant {
                path: self.paths.database_path().to_path_buf(),
                source,
            })?;
        if record.logs() != &self.log_paths(record.key()) {
            return Err(StorageError::InvalidRecordInvariant {
                path: self.paths.database_path().to_path_buf(),
                source: ProcessRecordValidationError::LogPaths,
            });
        }
        Ok(())
    }

    fn open_connection(&self) -> Result<Connection, StorageError> {
        self.paths.ensure_directories()?;
        let connection = Connection::open(self.paths.database_path()).map_err(|source| {
            StorageError::Sqlite {
                operation: "open SQLite database",
                source,
            }
        })?;
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|source| StorageError::Sqlite {
                operation: "enable SQLite foreign keys",
                source,
            })?;
        connection
            .busy_timeout(Duration::from_secs(1))
            .map_err(|source| StorageError::Sqlite {
                operation: "configure SQLite busy timeout",
                source,
            })?;
        Ok(connection)
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

#[derive(Debug)]
struct StoredRecord {
    digest: String,
    project_path: Vec<u8>,
    name: Vec<u8>,
    executable: Vec<u8>,
    pid: Option<i64>,
    process_group_id: Option<i64>,
    process_start_time: Option<String>,
    created_at: String,
    started_at: Option<String>,
    exited_at: Option<String>,
    state: String,
    exit_code: Option<i64>,
    termination_signal: Option<i64>,
    failure_reason: Option<String>,
}

impl StoredRecord {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            digest: row.get(0)?,
            project_path: row.get(1)?,
            name: row.get(2)?,
            executable: row.get(3)?,
            pid: row.get(4)?,
            process_group_id: row.get(5)?,
            process_start_time: row.get(6)?,
            created_at: row.get(7)?,
            started_at: row.get(8)?,
            exited_at: row.get(9)?,
            state: row.get(10)?,
            exit_code: row.get(11)?,
            termination_signal: row.get(12)?,
            failure_reason: row.get(13)?,
        })
    }
}

fn insert_record(transaction: &Transaction<'_>, record: &ProcessRecord) -> rusqlite::Result<()> {
    use std::os::unix::ffi::OsStrExt;

    let digest = files::key_digest(record.key());
    transaction.execute(
        "INSERT INTO process_records
            (key_digest, project_path, name, executable, pid, process_group_id,
             process_start_time, created_at, started_at, exited_at, state, exit_code,
             termination_signal, failure_reason)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            digest,
            project_bytes(record.key()),
            name_bytes(record.key()),
            record.executable().as_bytes(),
            record.pid().map(i64::from),
            record.process_group_id().map(i64::from),
            record.process_start_time().map(|value| value.to_string()),
            record.created_at().to_string(),
            record.started_at().map(|value| value.to_string()),
            record.exited_at().map(|value| value.to_string()),
            record.state().as_str(),
            record.exit_code().map(i64::from),
            record.termination_signal().map(i64::from),
            record.failure_reason(),
        ],
    )?;
    insert_arguments(transaction, &digest, record)
}

fn update_record(transaction: &Transaction<'_>, record: &ProcessRecord) -> rusqlite::Result<usize> {
    use std::os::unix::ffi::OsStrExt;

    let digest = files::key_digest(record.key());
    let changed = transaction.execute(
        "UPDATE process_records SET
            project_path = ?1, name = ?2, executable = ?3, pid = ?4,
            process_group_id = ?5, process_start_time = ?6, created_at = ?7,
            started_at = ?8, exited_at = ?9, state = ?10, exit_code = ?11,
            termination_signal = ?12, failure_reason = ?13
         WHERE key_digest = ?14",
        params![
            project_bytes(record.key()),
            name_bytes(record.key()),
            record.executable().as_bytes(),
            record.pid().map(i64::from),
            record.process_group_id().map(i64::from),
            record.process_start_time().map(|value| value.to_string()),
            record.created_at().to_string(),
            record.started_at().map(|value| value.to_string()),
            record.exited_at().map(|value| value.to_string()),
            record.state().as_str(),
            record.exit_code().map(i64::from),
            record.termination_signal().map(i64::from),
            record.failure_reason(),
            digest,
        ],
    )?;
    if changed != 0 {
        transaction.execute(
            "DELETE FROM process_arguments WHERE key_digest = ?1",
            params![digest],
        )?;
        insert_arguments(transaction, &digest, record)?;
    }
    Ok(changed)
}

fn insert_arguments(
    transaction: &Transaction<'_>,
    digest: &str,
    record: &ProcessRecord,
) -> rusqlite::Result<()> {
    use std::os::unix::ffi::OsStrExt;

    for (position, argument) in record.arguments().iter().enumerate() {
        transaction.execute(
            "INSERT INTO process_arguments (key_digest, position, value)
             VALUES (?1, ?2, ?3)",
            params![
                digest,
                i64::try_from(position).expect("argument position fits in SQLite integer"),
                argument.as_os_str().as_bytes(),
            ],
        )?;
    }
    Ok(())
}

fn load_record_with_connection(
    storage: &Storage,
    connection: &Connection,
    key: &ProcessKey,
) -> Result<Option<ProcessRecord>, StorageError> {
    let digest = files::key_digest(key);
    let stored = connection
        .query_row(
            "SELECT key_digest, project_path, name, executable, pid,
                    process_group_id, process_start_time, created_at, started_at,
                    exited_at, state, exit_code, termination_signal, failure_reason
             FROM process_records WHERE key_digest = ?1",
            params![digest],
            StoredRecord::from_row,
        )
        .optional()
        .map_err(|source| StorageError::Sqlite {
            operation: "load SQLite process record",
            source,
        })?;
    stored
        .map(|stored| {
            let arguments = load_arguments(connection, &stored.digest)?;
            decode_stored_record(storage, stored, arguments, Some(key))
        })
        .transpose()
}

fn load_arguments(
    connection: &Connection,
    digest: &str,
) -> Result<Vec<std::ffi::OsString>, StorageError> {
    use std::os::unix::ffi::OsStringExt;

    let mut statement = connection
        .prepare(
            "SELECT position, value FROM process_arguments
             WHERE key_digest = ?1 ORDER BY position",
        )
        .map_err(|source| StorageError::Sqlite {
            operation: "query SQLite process arguments",
            source,
        })?;
    let rows = statement
        .query_map(params![digest], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
        })
        .map_err(|source| StorageError::Sqlite {
            operation: "read SQLite process arguments",
            source,
        })?;
    let mut arguments = Vec::new();
    for row in rows {
        let (position, value) = row.map_err(|source| StorageError::Sqlite {
            operation: "read SQLite process argument row",
            source,
        })?;
        let expected =
            i64::try_from(arguments.len()).expect("argument count fits in SQLite integer");
        if position != expected {
            return Err(StorageError::InvalidStoredField {
                field: "argument position",
                value: position.to_string(),
            });
        }
        arguments.push(std::ffi::OsString::from_vec(value));
    }
    Ok(arguments)
}

fn decode_stored_record(
    storage: &Storage,
    stored: StoredRecord,
    arguments: Vec<std::ffi::OsString>,
    expected_key: Option<&ProcessKey>,
) -> Result<ProcessRecord, StorageError> {
    use std::os::unix::ffi::OsStringExt;

    if let Some(key) = expected_key {
        if stored.digest != files::key_digest(key)
            || stored.project_path != project_bytes(key)
            || stored.name != name_bytes(key)
        {
            return Err(StorageError::InvalidRecordInvariant {
                path: storage.paths.database_path().to_path_buf(),
                source: ProcessRecordValidationError::RecordPath,
            });
        }
    }

    let project_path = PathBuf::from(OsString::from_vec(stored.project_path));
    let key = ProcessKey::new(
        ProjectPath::from_canonical(project_path),
        OsString::from_vec(stored.name),
    );
    if stored.digest != files::key_digest(&key) {
        return Err(StorageError::InvalidRecordInvariant {
            path: storage.paths.database_path().to_path_buf(),
            source: ProcessRecordValidationError::RecordPath,
        });
    }
    let record = ProcessRecord::from_storage(ProcessRecordParts {
        key: key.clone(),
        executable: OsString::from_vec(stored.executable),
        arguments,
        pid: decode_u32(stored.pid, "pid")?,
        process_group_id: decode_u32(stored.process_group_id, "process group id")?,
        process_start_time: decode_u64(stored.process_start_time, "process start time")?,
        created_at: decode_required_u64(stored.created_at, "created_at")?,
        started_at: decode_u64(stored.started_at, "started_at")?,
        exited_at: decode_u64(stored.exited_at, "exited_at")?,
        state: stored
            .state
            .parse()
            .map_err(|value| StorageError::InvalidStoredField {
                field: "state",
                value,
            })?,
        exit_code: decode_i32(stored.exit_code, "exit code")?,
        termination_signal: decode_i32(stored.termination_signal, "termination signal")?,
        failure_reason: stored.failure_reason,
        logs: storage.log_paths(&key),
    })
    .map_err(|source| StorageError::InvalidRecordInvariant {
        path: storage.paths.database_path().to_path_buf(),
        source,
    })?;
    if let Some(expected_key) = expected_key {
        if record.key() != expected_key {
            return Err(StorageError::InvalidRecordInvariant {
                path: storage.paths.database_path().to_path_buf(),
                source: ProcessRecordValidationError::RecordPath,
            });
        }
    }
    Ok(record)
}

fn decode_u32(value: Option<i64>, field: &'static str) -> Result<Option<u32>, StorageError> {
    value
        .map(|value| {
            u32::try_from(value).map_err(|_| StorageError::InvalidStoredField {
                field,
                value: value.to_string(),
            })
        })
        .transpose()
}

fn decode_i32(value: Option<i64>, field: &'static str) -> Result<Option<i32>, StorageError> {
    value
        .map(|value| {
            i32::try_from(value).map_err(|_| StorageError::InvalidStoredField {
                field,
                value: value.to_string(),
            })
        })
        .transpose()
}

fn decode_u64(value: Option<String>, field: &'static str) -> Result<Option<u64>, StorageError> {
    value
        .map(|value| {
            value
                .parse()
                .map_err(|_| StorageError::InvalidStoredField { field, value })
        })
        .transpose()
}

fn decode_required_u64(value: String, field: &'static str) -> Result<u64, StorageError> {
    value
        .parse()
        .map_err(|_| StorageError::InvalidStoredField { field, value })
}

#[cfg(unix)]
fn project_bytes(key: &ProcessKey) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    key.project_path().as_os_str().as_bytes().to_vec()
}

#[cfg(unix)]
fn name_bytes(key: &ProcessKey) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    key.name().as_bytes().to_vec()
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
