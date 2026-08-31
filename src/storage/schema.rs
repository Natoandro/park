use rusqlite::Connection;

use super::StorageError;

const SCHEMA_VERSION: i32 = 1;

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

    create(connection)?;
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
