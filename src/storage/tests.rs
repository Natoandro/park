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
    let paths = StoragePaths::from_environment(&XdgEnvironment {
        state_home: Some(root.path().join("state")),
        runtime_dir: Some(root.path().join("runtime")),
        home: None,
    })
    .expect("explicit XDG paths should resolve");
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
        state_home: Some(PathBuf::from("/state")),
        runtime_dir: Some(PathBuf::from("/run/user/1000")),
        home: Some(PathBuf::from("/home/user")),
    })
    .expect("explicit paths should resolve");
    assert_eq!(explicit.state_dir(), PathBuf::from("/state/park"));
    assert_eq!(explicit.runtime_dir(), PathBuf::from("/run/user/1000/park"));

    let fallback = StoragePaths::from_environment(&XdgEnvironment {
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
    let name = OsString::from_vec(vec![b'd', 0xff]);
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
fn rejects_a_record_with_external_log_paths_before_removal() {
    let (root, storage, project) = test_storage();
    let mut record = record(&storage, &project, OsString::from("dev"));
    record
        .mark_spawn_failed(11, "could not start")
        .expect("record should become failed");
    storage
        .create_record(&record)
        .expect("record should be created");
    let external_log = root.path().join("outside.log");
    fs::write(&external_log, b"must remain").expect("external log should exist");

    let path = storage.paths().database_path();
    let mut value: serde_json::Value =
        serde_json::to_value(&record).expect("record should serialize");
    value["logs"]["stdout"] = serde_json::json!(external_log);
    let connection = rusqlite::Connection::open(path).expect("database should open");
    connection
        .execute(
            "UPDATE process_records SET record_json = ?1 WHERE key_digest = ?2",
            rusqlite::params![
                serde_json::to_vec(&value).expect("record should serialize"),
                files::key_digest(record.key()),
            ],
        )
        .expect("record should be corrupted");

    assert!(matches!(
        storage.remove_record(record.key(), false),
        Err(StorageError::InvalidRecordInvariant { .. })
    ));
    assert_eq!(
        fs::read(&external_log).expect("external log should remain"),
        b"must remain"
    );
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
