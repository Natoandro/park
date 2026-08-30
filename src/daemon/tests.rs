use super::*;
use crate::ipc::{IpcOperation, IpcRequest};
use crate::project::ProjectPath;
use std::ffi::OsString;

#[test]
fn only_one_owner_can_hold_the_daemon_lock() {
    let root = std::env::temp_dir().join(format!("park-phase4-lock-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let paths = StoragePaths::from_environment(&crate::storage::XdgEnvironment {
        state_home: Some(root.join("state")),
        runtime_dir: Some(root.join("runtime")),
        home: None,
    })
    .expect("paths should resolve");
    let first = DaemonLock::try_acquire(paths.clone())
        .expect("first lock should work")
        .expect("first owner should acquire lock");
    assert!(
        DaemonLock::try_acquire(paths.clone())
            .expect("second lock attempt should work")
            .is_none()
    );
    drop(first);
    assert!(
        DaemonLock::try_acquire(paths)
            .expect("reacquire should work")
            .is_some()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_unknown_protocol_versions() {
    let paths = test_paths();
    let storage = Storage::new(paths);
    let response = handle_request(
        &storage,
        IpcRequest {
            version: PROTOCOL_VERSION + 1,
            request_id: 9,
            operation: IpcOperation::Status {
                key: crate::process::ProcessKey::new(
                    ProjectPath::from_canonical("/project".into()),
                    OsString::from("dev"),
                ),
            },
        },
    );
    assert_eq!(response.result.status, ResultStatus::Failure);
}

#[test]
fn ps_and_status_return_persisted_records() {
    let root = std::env::temp_dir().join(format!("park-phase4-handler-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).expect("test root should be created");
    let project_path = root.join("project");
    fs::create_dir(&project_path).expect("project directory should be created");
    let project = ProjectPath::from_canonical(project_path.clone());
    let storage = Storage::new(
        StoragePaths::from_environment(&crate::storage::XdgEnvironment {
            state_home: Some(root.join("state")),
            runtime_dir: Some(root.join("runtime")),
            home: None,
        })
        .expect("paths should resolve"),
    );
    let key = crate::process::ProcessKey::new(project.clone(), OsString::from("dev"));
    let record = crate::process::ProcessRecord::new(
        key.clone(),
        project_path,
        OsString::from("server"),
        vec![OsString::from("--dev")],
        1,
        storage.create_logs(&key).expect("logs should be created"),
    );
    storage
        .create_record(&record)
        .expect("record should be persisted");

    let ps = handle_request(
        &storage,
        IpcRequest {
            version: PROTOCOL_VERSION,
            request_id: 1,
            operation: IpcOperation::Ps {
                project_path: project,
            },
        },
    );
    assert_eq!(ps.result.status, ResultStatus::Success);
    assert_eq!(
        ps.result
            .data
            .expect("ps data should exist")
            .as_array()
            .map(Vec::len),
        Some(1)
    );

    let status = handle_request(
        &storage,
        IpcRequest {
            version: PROTOCOL_VERSION,
            request_id: 2,
            operation: IpcOperation::Status { key },
        },
    );
    assert_eq!(status.result.status, ResultStatus::Success);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn invalid_process_identifiers_are_never_considered_live() {
    let paths = test_paths();
    let storage = Storage::new(paths);
    let project = ProjectPath::from_canonical("/project".into());
    let key = crate::process::ProcessKey::new(project.clone(), OsString::from("dev"));
    let record = crate::process::ProcessRecord::new(
        key,
        project.into_path(),
        OsString::from("server"),
        vec![],
        1,
        storage.log_paths(&crate::process::ProcessKey::new(
            ProjectPath::from_canonical("/project".into()),
            OsString::from("dev"),
        )),
    );
    let mut value = serde_json::to_value(record).expect("record should serialize");
    value["state"] = serde_json::json!("running");
    value["pid"] = serde_json::json!(u32::MAX);
    value["process_group_id"] = serde_json::json!(u32::MAX);
    value["process_start_time"] = serde_json::json!(1);
    let record = serde_json::from_value(value).expect("record should deserialize");

    assert!(!record_is_alive(&record));
}

fn test_paths() -> StoragePaths {
    StoragePaths::from_environment(&crate::storage::XdgEnvironment {
        state_home: Some("/state".into()),
        runtime_dir: Some("/runtime".into()),
        home: None,
    })
    .expect("paths should resolve")
}
