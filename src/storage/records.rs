use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior};

use crate::process::{
    ProcessKey, ProcessRecord, ProcessRecordValidationError, validate_process_name,
};

use super::record_codec::{
    StoredRecord, decode_stored_record, insert_record, load_arguments, load_record_with_connection,
    update_record,
};
use super::{Storage, StorageError};

impl Storage {
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
        let mut records = Vec::new();
        for stored in stored_records {
            if validate_process_name(&OsString::from_vec(stored.name_bytes().to_vec())).is_err() {
                continue;
            }
            let arguments = load_arguments(&connection, &stored.digest)?;
            records.push(decode_stored_record(self, stored, arguments, None)?);
        }
        Ok(records)
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

    pub(super) fn open_connection(&self) -> Result<Connection, StorageError> {
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
}
