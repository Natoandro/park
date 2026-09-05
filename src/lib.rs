mod cli;
mod client;
mod config;
mod daemon;
mod environment;
mod help;
mod ipc;
mod lifecycle;
pub(crate) mod os_string;
mod process;
mod project;
mod registry;
mod result;
mod storage;

pub use cli::{
    DaemonOperation, EnvArgs, Invocation, LogsArgs, Operation, PsScope, RestartArgs, StartArgs,
    WaitArgs, parse_invocation,
};
pub use client::{ClientError, request_with_daemon_start, stream_request_with_daemon_start};
pub use config::{
    ActiveProcessPolicy, Config, ConfigError, DaemonConfig, ManagedProcessesConfig, ReexecConfig,
    RestartBackoff, RestartConfig, RestartPolicy, config_path,
};
pub use daemon::descriptors::{
    DescriptorEntry, DescriptorError, DescriptorRole, DescriptorTable, MAX_INHERITED_FD,
    MIN_INHERITED_FD,
};
pub use daemon::handoff::{
    HANDOFF_VERSION, HandoffError, HandoffManifest, MANIFEST_FILE_NAME, MAX_MANIFEST_BYTES,
    manifest_path,
};
pub use daemon::{
    DaemonError, DaemonPhase, INTERNAL_DAEMON_ARGUMENT, INTERNAL_SUPERVISOR_ARGUMENT,
    run as run_daemon,
};
pub use environment::{
    EnvironmentCapture, EnvironmentEntry, EnvironmentError, EnvironmentOverride, EnvironmentSpec,
    ResolvedEnvironment,
};
pub use help::{command_help_result, skills_help_result};
pub use ipc::{
    IpcError, IpcLogOptions, IpcOperation, IpcRequest, IpcResponse, RecaptureEnvironment,
    request_for_clean, request_for_daemon_config, request_for_daemon_status, request_for_env,
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
