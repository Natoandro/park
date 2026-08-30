mod cli;
mod client;
mod daemon;
mod ipc;
mod lifecycle;
mod process;
mod project;
mod registry;
mod result;
mod storage;

pub use cli::{Invocation, LogsArgs, Operation, WaitArgs, parse_invocation};
pub use client::{ClientError, request_with_daemon_start, stream_request_with_daemon_start};
pub use daemon::{
    DaemonError, INTERNAL_DAEMON_ARGUMENT, INTERNAL_SUPERVISOR_ARGUMENT, run as run_daemon,
};
pub use ipc::{
    IpcError, IpcLogOptions, IpcOperation, IpcRequest, IpcResponse, request_for_launch,
    request_for_logs, request_for_ps, request_for_status,
};
pub use lifecycle::{
    InvalidLifecycleAction, InvalidStateTransition, LifecycleAction, ProcessState,
};
pub use process::{
    EpochSeconds, LogPaths, ProcessKey, ProcessRecord, ProcessRecordValidationError,
};
pub use project::{ProjectPath, ProjectResolutionError, resolve_current_project, resolve_project};
pub use registry::{ProcessRegistry, RegistryError};
pub use result::{CommandResult, RenderError, ResultError, ResultStatus, render_json};
pub use storage::{Storage, StorageError, StoragePaths, XdgEnvironment};
