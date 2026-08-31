use std::ffi::OsString;
use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};

use crate::process::{ProcessKey, ProcessRecord, ProcessRecordParts, ProcessRecordValidationError};
use crate::project::ProjectPath;

use super::{Storage, StorageError, files};

#[derive(Debug)]
pub(super) struct StoredRecord {
    pub digest: String,
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
    pub(super) fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
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

pub(super) fn insert_record(
    transaction: &Transaction<'_>,
    record: &ProcessRecord,
) -> rusqlite::Result<()> {
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

pub(super) fn update_record(
    transaction: &Transaction<'_>,
    record: &ProcessRecord,
) -> rusqlite::Result<usize> {
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

pub(super) fn load_record_with_connection(
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

pub(super) fn load_arguments(
    connection: &Connection,
    digest: &str,
) -> Result<Vec<OsString>, StorageError> {
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
        arguments.push(OsString::from_vec(value));
    }
    Ok(arguments)
}

pub(super) fn decode_stored_record(
    storage: &Storage,
    stored: StoredRecord,
    arguments: Vec<OsString>,
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

fn project_bytes(key: &ProcessKey) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    key.project_path().as_os_str().as_bytes().to_vec()
}

fn name_bytes(key: &ProcessKey) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    key.name().as_bytes().to_vec()
}
