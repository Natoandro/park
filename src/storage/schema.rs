use rusqlite::{Connection, params};

use super::StorageError;

const SCHEMA_VERSION: i32 = 2;

pub(super) fn initialize(connection: &mut Connection) -> Result<(), StorageError> {
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

    if version == 1 {
        migrate_v1(connection)?;
    }
    create(connection)?;
    if version != SCHEMA_VERSION {
        connection
            .execute_batch("PRAGMA user_version = 2;")
            .map_err(|source| StorageError::Sqlite {
                operation: "set SQLite schema version",
                source,
            })?;
    }
    Ok(())
}

fn create(connection: &Connection) -> Result<(), StorageError> {
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
                 failure_reason TEXT,
                 environment_capture BLOB NOT NULL DEFAULT X'5B5D',
                 dotenv_files BLOB NOT NULL DEFAULT X'5B5D',
                 environment_overrides BLOB NOT NULL DEFAULT X'5B5D'
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

fn migrate_v1(connection: &Connection) -> Result<(), StorageError> {
    connection
        .execute_batch(
            "ALTER TABLE process_records ADD COLUMN environment_capture BLOB NOT NULL DEFAULT X'5B5D';
             ALTER TABLE process_records ADD COLUMN dotenv_files BLOB NOT NULL DEFAULT X'5B5D';
             ALTER TABLE process_records ADD COLUMN environment_overrides BLOB NOT NULL DEFAULT X'5B5D';",
        )
        .map_err(|source| StorageError::Sqlite {
            operation: "migrate SQLite environment schema",
            source,
        })?;
    let capture = crate::environment::EnvironmentCapture::from_process().map_err(|error| {
        StorageError::InvalidStoredField {
            field: "legacy environment capture",
            value: error.to_string(),
        }
    })?;
    let capture =
        serde_json::to_vec(&capture).map_err(|error| StorageError::InvalidStoredField {
            field: "legacy environment capture",
            value: error.to_string(),
        })?;
    connection
        .execute(
            "UPDATE process_records SET environment_capture = ?1",
            params![capture],
        )
        .map_err(|source| StorageError::Sqlite {
            operation: "backfill legacy environment capture",
            source,
        })?;
    Ok(())
}
