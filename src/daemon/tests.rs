use super::*;
use crate::ipc::{IpcOperation, IpcRequest};
use crate::project::ProjectPath;
use std::ffi::OsString;

#[test]
fn only_one_owner_can_hold_the_daemon_lock() {
    let root = std::env::temp_dir().join(format!("park-phase4-lock-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let paths = StoragePaths::from_environment(&crate::storage::XdgEnvironment {
        config_home: None,
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
    let mut request = IpcRequest::new(
        9,
        IpcOperation::Status {
            key: crate::process::ProcessKey::new(
                ProjectPath::from_canonical("/project".into()),
                OsString::from("dev"),
            ),
        },
    );
    request.version = PROTOCOL_VERSION + 1;
    let response = handle_request(&storage, request);
    assert_eq!(response.result.status, ResultStatus::Failure);
}

#[test]
fn rejects_mismatched_client_versions() {
    let root = std::env::temp_dir().join(format!("park-client-version-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let paths = StoragePaths::from_environment(&crate::storage::XdgEnvironment {
        config_home: None,
        state_home: Some(root.join("state")),
        runtime_dir: Some(root.join("runtime")),
        home: None,
    })
    .expect("paths should resolve");
    paths
        .ensure_directories()
        .expect("storage should initialize");
    let storage = Storage::new(paths);
    let mut request = IpcRequest::new(0, IpcOperation::Ping);
    request.client_version = "0.0.0".to_owned();
    let mismatch = handle_request(&storage, request);
    assert_eq!(mismatch.result.status, ResultStatus::Failure);
    assert!(
        mismatch
            .result
            .human_message()
            .contains("incompatible Park versions")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn recognizes_reexec_as_an_internal_operation() {
    let root = std::env::temp_dir().join(format!("park-reexec-operation-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let paths = StoragePaths::from_environment(&crate::storage::XdgEnvironment {
        config_home: None,
        state_home: Some(root.join("state")),
        runtime_dir: Some(root.join("runtime")),
        home: None,
    })
    .expect("paths should resolve");
    paths
        .ensure_directories()
        .expect("storage should initialize");
    let storage = Storage::new(paths);
    let response = handle_request(
        &storage,
        IpcRequest::new(
            4,
            IpcOperation::Reexec {
                candidate_path: "/usr/local/bin/park".into(),
                candidate_version: "0.3.0".to_owned(),
            },
        ),
    );
    assert_eq!(response.result.status, ResultStatus::Failure);
    assert_eq!(
        response.result.human_message(),
        "reexec requests are not implemented"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn blocks_mutating_and_streaming_operations_during_quiescing() {
    assert!(operation_is_blocked_during_quiesce(&IpcOperation::Clean));
    assert!(operation_is_blocked_during_quiesce(&IpcOperation::Logs {
        key: crate::process::ProcessKey::new(
            ProjectPath::from_canonical("/project".into()),
            OsString::from("dev"),
        ),
        tail: None,
        head: None,
        follow: true,
        grep: None,
        stdout: false,
        stderr: false,
    }));
    assert!(!operation_is_blocked_during_quiesce(&IpcOperation::Ping));
    assert!(!operation_is_blocked_during_quiesce(
        &IpcOperation::DaemonStatus
    ));
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
            config_home: None,
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
        IpcRequest::new(
            1,
            IpcOperation::Ps {
                project_path: project,
            },
        ),
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

    let status = handle_request(&storage, IpcRequest::new(2, IpcOperation::Status { key }));
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

#[cfg(unix)]
#[test]
fn canonicalizes_daemon_project_paths_before_dispatch() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!("park-daemon-project-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).expect("test root should be created");
    let project = root.join("project");
    let alias = root.join("project-alias");
    fs::create_dir(&project).expect("project should be created");
    symlink(&project, &alias).expect("project alias should be created");

    let operation = canonicalize_operation(IpcOperation::Ps {
        project_path: ProjectPath::from_canonical(alias),
    })
    .expect("alias should resolve");
    assert!(matches!(
        operation,
        IpcOperation::Ps { project_path } if project_path.as_path() == fs::canonicalize(&project).expect("project should canonicalize")
    ));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_invalid_daemon_project_paths() {
    assert!(
        canonicalize_operation(IpcOperation::Ps {
            project_path: ProjectPath::from_canonical("relative-project".into()),
        })
        .is_err()
    );
}

fn test_paths() -> StoragePaths {
    StoragePaths::from_environment(&crate::storage::XdgEnvironment {
        config_home: None,
        state_home: Some("/state".into()),
        runtime_dir: Some("/runtime".into()),
        home: None,
    })
    .expect("paths should resolve")
}
