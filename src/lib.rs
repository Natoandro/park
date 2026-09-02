mod cli;
mod client;
mod daemon;
mod help;
mod ipc;
mod lifecycle;
pub(crate) mod os_string;
mod process;
mod project;
mod registry;
mod result;
mod storage;

pub use cli::{DaemonOperation, Invocation, LogsArgs, Operation, WaitArgs, parse_invocation};
pub use client::{ClientError, request_with_daemon_start, stream_request_with_daemon_start};
pub use daemon::{
    DaemonError, INTERNAL_DAEMON_ARGUMENT, INTERNAL_SUPERVISOR_ARGUMENT, run as run_daemon,
};
pub use help::skills_help_result;
pub use ipc::{
    IpcError, IpcLogOptions, IpcOperation, IpcRequest, IpcResponse, request_for_clean,
    request_for_launch, request_for_logs, request_for_ps, request_for_reexec, request_for_remove,
    request_for_restart, request_for_signal, request_for_start, request_for_status,
    request_for_stop, request_for_wait,
};
pub use lifecycle::{
    InvalidLifecycleAction, InvalidStateTransition, LifecycleAction, ProcessState,
};
pub use process::{
    EpochSeconds, LogPaths, ProcessKey, ProcessNameError, ProcessRecord,
    ProcessRecordValidationError, validate_process_name,
};
pub use project::{ProjectPath, ProjectResolutionError, resolve_current_project, resolve_project};
pub use registry::{ProcessRegistry, RegistryError};
pub use result::{CommandResult, RenderError, ResultError, ResultStatus, render_json};
pub use storage::{Storage, StorageError, StoragePaths, XdgEnvironment};
