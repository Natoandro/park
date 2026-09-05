use super::*;
use crate::{ProcessKey, ProcessState, ProjectPath, resolve_project};
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let id = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("park-phase3-{}-{id}", std::process::id()));
        fs::create_dir(&path).expect("temporary directory should be created");
        Self(path)
    }

    fn path(&self) -> &PathBuf {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn test_storage() -> (TempDir, Storage, ProjectPath) {
    let root = TempDir::new();
    let paths = test_paths(&root);
    let storage = Storage::new(paths);
    storage
        .paths()
        .ensure_directories()
        .expect("storage directories should be created");

    let project_dir = root.path().join("project");
    fs::create_dir(&project_dir).expect("project directory should be created");
    let project = resolve_project(project_dir).expect("project should resolve");
    (root, storage, project)
}

fn test_paths(root: &TempDir) -> StoragePaths {
    StoragePaths::from_environment(&XdgEnvironment {
        config_home: None,
        state_home: Some(root.path().join("state")),
        runtime_dir: Some(root.path().join("runtime")),
        home: None,
    })
    .expect("explicit XDG paths should resolve")
}

fn record(storage: &Storage, project: &ProjectPath, name: OsString) -> ProcessRecord {
    let key = ProcessKey::new(project.clone(), name);
    let logs = storage
        .create_logs(&key)
        .expect("separate log files should be created");
    ProcessRecord::new(
        key,
        project.as_path().to_path_buf(),
        OsString::from("server"),
        vec![OsString::from("--dev")],
        10,
        logs,
    )
}

#[test]
fn resolves_explicit_xdg_paths_and_safe_fallbacks() {
    let explicit = StoragePaths::from_environment(&XdgEnvironment {
        config_home: None,
        state_home: Some(PathBuf::from("/state")),
        runtime_dir: Some(PathBuf::from("/run/user/1000")),
        home: Some(PathBuf::from("/home/user")),
    })
    .expect("explicit paths should resolve");
    assert_eq!(explicit.state_dir(), PathBuf::from("/state/park"));
    assert_eq!(explicit.runtime_dir(), PathBuf::from("/run/user/1000/park"));

    let fallback = StoragePaths::from_environment(&XdgEnvironment {
        config_home: None,
        state_home: None,
        runtime_dir: None,
        home: Some(PathBuf::from("/home/user")),
    })
    .expect("HOME fallback should resolve");
    assert_eq!(
        fallback.state_dir(),
        PathBuf::from("/home/user/.local/state/park")
    );
    assert_eq!(
        fallback.runtime_dir(),
        PathBuf::from("/home/user/.local/state/park/runtime/park")
    );

    assert!(matches!(
        StoragePaths::from_environment(&XdgEnvironment {
            config_home: None,
            state_home: None,
            runtime_dir: None,
            home: None,
        }),
        Err(StorageError::MissingHome)
    ));
}

#[test]
fn creates_separate_logs_and_safe_encoded_paths() {
    let (_root, storage, project) = test_storage();
    let record = record(&storage, &project, OsString::from("../../service"));
    let logs = storage.log_paths(record.key());

    assert!(storage.paths().database_path().exists());
    assert!(
        storage
            .paths()
            .database_path()
            .starts_with(storage.paths().state_dir())
    );
    assert_ne!(logs.stdout, logs.stderr);
    assert!(logs.stdout.exists());
    assert!(logs.stderr.exists());
}

#[cfg(unix)]
#[test]
fn round_trips_exact_non_utf8_process_arguments() {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    let (_root, storage, project) = test_storage();
    let name = OsString::from("non-utf8-args");
    let key = ProcessKey::new(project.clone(), name);
    let logs = storage.create_logs(&key).expect("logs should be created");
    let mut record = ProcessRecord::new(
        key.clone(),
        project.as_path().to_path_buf(),
        OsString::from_vec(vec![b's', 0xfe]),
        vec![OsString::from_vec(vec![b'a', 0xfd])],
        10,
        logs,
    );
    record
        .mark_running(11, 123, Some(123), Some(123))
        .expect("record should become running");
    storage
        .create_record(&record)
        .expect("record should be persisted");

    let loaded = storage
        .load_record(&key)
        .expect("record should load")
        .expect("record should exist");
    assert_eq!(loaded.key(), &key);
    assert_eq!(loaded.executable().as_bytes(), &[b's', 0xfe]);
    assert_eq!(loaded.arguments()[0].as_bytes(), &[b'a', 0xfd]);
}

#[cfg(unix)]
#[test]
fn treats_persisted_records_with_invalid_names_as_absent() {
    use std::os::unix::ffi::OsStringExt;

    let (_root, storage, project) = test_storage();
    let name = OsString::from_vec(b"legacy-\xff".to_vec());
    let record = record(&storage, &project, name.clone());
    let key = record.key().clone();
    storage
        .create_record(&record)
        .expect("record should be persisted");

    assert!(
        storage
            .load_record(&key)
            .expect("record lookup should succeed")
            .is_none()
    );
    assert!(
        storage
            .list_records()
            .expect("record listing should succeed")
            .is_empty()
    );
}

#[test]
fn stores_records_in_normalized_tables() {
    let (_root, storage, project) = test_storage();
    let record = record(&storage, &project, OsString::from("dev"));
    storage
        .create_record(&record)
        .expect("record should be persisted");

    let connection =
        rusqlite::Connection::open(storage.paths().database_path()).expect("database should open");
    let columns = connection
        .prepare("SELECT name FROM pragma_table_info('process_records')")
        .expect("record schema should be inspectable")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("record columns should be readable")
        .collect::<Result<Vec<_>, _>>()
        .expect("record columns should load");
    assert!(columns.contains(&"executable".to_owned()));
    let version: i32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("schema version should be readable");
    assert_eq!(version, 2);
    let argument_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM process_arguments WHERE key_digest = ?1",
            rusqlite::params![files::key_digest(record.key())],
            |row| row.get(0),
        )
        .expect("argument rows should be countable");
    assert_eq!(argument_count, 1);
}

#[test]
fn migrates_version_one_databases_for_environment_inputs() {
    let root = TempDir::new();
    let paths = test_paths(&root);
    fs::create_dir_all(paths.state_dir()).expect("state directory should be created");
    let connection =
        rusqlite::Connection::open(paths.database_path()).expect("database should open");
    connection
        .execute_batch(
            "CREATE TABLE process_records (
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
            CREATE TABLE process_arguments (
                key_digest TEXT NOT NULL,
                position INTEGER NOT NULL,
                value BLOB NOT NULL,
                PRIMARY KEY(key_digest, position)
            );
            PRAGMA user_version = 1;",
        )
        .expect("version one schema should be created");
    drop(connection);

    paths
        .ensure_directories()
        .expect("version one schema should migrate");
    let connection =
        rusqlite::Connection::open(paths.database_path()).expect("database should open");
    let version: i32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("schema version should be readable");
    assert_eq!(version, 2);
    let columns: i64 = connection
        .query_row(
            "SELECT count(*) FROM pragma_table_info('process_records')
             WHERE name IN ('environment_capture', 'dotenv_files', 'environment_overrides')",
            [],
            |row| row.get(0),
        )
        .expect("environment columns should be present");
    assert_eq!(columns, 3);
}

#[test]
fn atomically_persists_and_replaces_records() {
    let (_root, storage, project) = test_storage();
    let mut record = record(&storage, &project, OsString::from("dev"));
    storage
        .create_record(&record)
        .expect("record should be created");
    assert!(matches!(
        storage.create_record(&record),
        Err(StorageError::RecordExists { .. })
    ));

    record
        .mark_running(11, 123, Some(123), Some(123))
        .expect("record should become running");
    storage
        .save_record(&record)
        .expect("record should be atomically replaced");
    assert_eq!(
        storage
            .load_record(record.key())
            .expect("record should load")
            .expect("record should exist")
            .state(),
        ProcessState::Running
    );

    assert_eq!(
        storage.list_records().expect("records should list").len(),
        1
    );
}

#[test]
fn conditional_record_saves_reject_stale_observations() {
    let (_root, storage, project) = test_storage();
    let record = record(&storage, &project, OsString::from("dev"));
    storage
        .create_record(&record)
        .expect("record should be created");

    let mut running = record.clone();
    running
        .mark_running(11, 123, Some(123), Some(123))
        .expect("record should become running");
    assert!(
        storage
            .save_record_if_unchanged(&record, &running)
            .expect("conditional save should succeed")
    );

    let mut stale = record.clone();
    stale
        .mark_spawn_failed(12, "stale update")
        .expect("stale record should be locally valid");
    assert!(
        !storage
            .save_record_if_unchanged(&record, &stale)
            .expect("stale conditional save should be checked")
    );
    assert_eq!(
        storage
            .load_record(running.key())
            .expect("record should load")
            .expect("record should exist")
            .state(),
        ProcessState::Running
    );
}

#[test]
fn retains_logs_and_rejects_removal_of_active_records() {
    let (_root, storage, project) = test_storage();
    let mut record = record(&storage, &project, OsString::from("dev"));
    storage
        .create_record(&record)
        .expect("record should be created");
    assert!(matches!(
        storage.remove_record(record.key(), true),
        Err(StorageError::ActiveRecord { .. })
    ));

    record
        .mark_running(11, 123, Some(123), Some(123))
        .expect("record should become running");
    record
        .mark_terminated(12, Some(0), None)
        .expect("record should become terminal");
    storage
        .save_record(&record)
        .expect("terminal record should save");
    let logs = record.logs().clone();
    storage
        .remove_record(record.key(), true)
        .expect("terminal record should be removable");
    assert!(
        storage
            .load_record(record.key())
            .expect("record lookup should succeed")
            .is_none()
    );
    assert!(logs.stdout.exists());
    assert!(logs.stderr.exists());
}

#[test]
fn reconciles_dead_active_records_without_discarding_logs() {
    let (_root, storage, project) = test_storage();
    let mut record = record(&storage, &project, OsString::from("dev"));
    record
        .mark_running(11, 123, Some(123), Some(123))
        .expect("record should become running");
    let logs = record.logs().clone();
    storage
        .create_record(&record)
        .expect("record should be created");

    let reconciled = storage
        .reconcile(20, |_| false)
        .expect("reconciliation should succeed");
    assert_eq!(reconciled, vec![record.key().clone()]);
    let loaded = storage
        .load_record(record.key())
        .expect("record should load")
        .expect("record should exist");
    assert_eq!(loaded.state(), ProcessState::Exited);
    assert_eq!(loaded.exited_at(), Some(20));
    assert!(logs.stdout.exists());
    assert!(logs.stderr.exists());
}

#[test]
fn rejects_a_record_with_invalid_stored_fields() {
    let (_root, storage, project) = test_storage();
    let mut record = record(&storage, &project, OsString::from("dev"));
    record
        .mark_spawn_failed(11, "could not start")
        .expect("record should become failed");
    storage
        .create_record(&record)
        .expect("record should be created");
    let path = storage.paths().database_path();
    let connection = rusqlite::Connection::open(path).expect("database should open");
    connection
        .execute(
            "UPDATE process_records SET executable = ?1 WHERE key_digest = ?2",
            rusqlite::params![b"".as_slice(), files::key_digest(record.key())],
        )
        .expect("record should be corrupted");

    assert!(matches!(
        storage.remove_record(record.key(), false),
        Err(StorageError::InvalidRecordInvariant { .. })
    ));
}

#[test]
fn rejects_a_record_with_mismatched_identity_columns() {
    let (_root, storage, project) = test_storage();
    let record = record(&storage, &project, OsString::from("dev"));
    storage
        .create_record(&record)
        .expect("record should be created");
    let connection =
        rusqlite::Connection::open(storage.paths().database_path()).expect("database should open");
    connection
        .execute(
            "UPDATE process_records SET name = ?1 WHERE key_digest = ?2",
            rusqlite::params![b"different".as_slice(), files::key_digest(record.key())],
        )
        .expect("record identity should be corrupted");

    assert!(matches!(
        storage.list_records(),
        Err(StorageError::InvalidRecordInvariant { .. })
    ));
}
