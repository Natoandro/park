use std::ffi::OsString;

use clap::{Args, Parser, Subcommand};
use serde::Serialize;
use thiserror::Error;

/// The operation requested by a user, before a daemon handles it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    Launch {
        name: OsString,
        command: Vec<OsString>,
    },
    Operation(Operation),
}

impl Invocation {
    pub fn requests_json(&self) -> bool {
        match self {
            Self::Launch { .. } => false,
            Self::Operation(operation) => operation.requests_json(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    Ps { json: bool },
    Status { name: OsString, json: bool },
    Logs(LogsArgs),
    Stop { name: OsString, force: bool },
    Restart { name: OsString },
    Start { name: OsString },
    Signal { name: OsString, signal: String },
    Rm { name: OsString, keep_logs: bool },
    Clean,
    Wait(WaitArgs),
}

impl Operation {
    fn requests_json(&self) -> bool {
        match self {
            Self::Ps { json } | Self::Status { json, .. } => *json,
            Self::Logs(args) => args.json,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogsArgs {
    pub name: OsString,
    pub tail: Option<u64>,
    pub head: Option<u64>,
    pub follow: bool,
    pub grep: Option<String>,
    pub stdout: bool,
    pub stderr: bool,
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitArgs {
    pub name: OsString,
    pub state: Option<String>,
    pub match_text: Option<String>,
    pub exit: bool,
    pub timeout: Option<String>,
}

#[derive(Debug, Parser)]
#[command(name = "park", version, about = "Project-scoped local process manager")]
struct LaunchCli {
    #[arg(value_name = "NAME", allow_hyphen_values = true)]
    name: OsString,

    #[arg(last = true, required = true, value_name = "COMMAND")]
    command: Vec<OsString>,
}

#[derive(Debug, Parser)]
#[command(name = "park run", version, about = "Start a named command")]
struct RunCli {
    #[arg(value_name = "NAME", allow_hyphen_values = true)]
    name: OsString,

    #[arg(last = true, required = true, value_name = "COMMAND")]
    command: Vec<OsString>,
}

#[derive(Debug, Parser)]
#[command(name = "park", version, about = "Project-scoped local process manager")]
struct OperationCli {
    #[command(subcommand)]
    operation: OperationCliCommand,
}

#[derive(Debug, Subcommand)]
enum OperationCliCommand {
    #[command(name = "ps")]
    Ps {
        #[arg(long)]
        json: bool,
    },
    #[command(name = "status")]
    Status {
        #[arg(value_name = "NAME", allow_hyphen_values = true)]
        name: OsString,
        #[arg(long)]
        json: bool,
    },
    #[command(name = "logs")]
    Logs(LogsCliArgs),
    #[command(name = "stop")]
    Stop {
        #[arg(value_name = "NAME", allow_hyphen_values = true)]
        name: OsString,
        #[arg(long)]
        force: bool,
    },
    #[command(name = "restart")]
    Restart {
        #[arg(value_name = "NAME", allow_hyphen_values = true)]
        name: OsString,
    },
    #[command(name = "start")]
    Start {
        #[arg(value_name = "NAME", allow_hyphen_values = true)]
        name: OsString,
    },
    #[command(name = "signal")]
    Signal {
        #[arg(value_name = "NAME", allow_hyphen_values = true)]
        name: OsString,
        #[arg(value_name = "SIGNAL")]
        signal: String,
    },
    #[command(name = "rm")]
    Rm {
        #[arg(value_name = "NAME", allow_hyphen_values = true)]
        name: OsString,
        #[arg(long)]
        keep_logs: bool,
    },
    #[command(name = "clean")]
    Clean,
    #[command(name = "wait")]
    Wait(WaitCliArgs),
}

#[derive(Debug, Args)]
struct LogsCliArgs {
    #[arg(value_name = "NAME", allow_hyphen_values = true)]
    name: OsString,
    #[arg(long, value_name = "N")]
    tail: Option<u64>,
    #[arg(long, value_name = "N")]
    head: Option<u64>,
    #[arg(long)]
    follow: bool,
    #[arg(long = "grep", value_name = "PATTERN")]
    grep: Option<String>,
    #[arg(long, conflicts_with = "stderr")]
    stdout: bool,
    #[arg(long, conflicts_with = "stdout")]
    stderr: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct WaitCliArgs {
    #[arg(value_name = "NAME", allow_hyphen_values = true)]
    name: OsString,
    #[arg(long, value_name = "STATE", group = "wait-condition")]
    state: Option<String>,
    #[arg(long = "match", value_name = "TEXT", group = "wait-condition")]
    match_text: Option<String>,
    #[arg(long, group = "wait-condition")]
    exit: bool,
    #[arg(long, value_name = "DURATION")]
    timeout: Option<String>,
}

/// Parse the public CLI grammar without imposing lexical restrictions on names.
///
/// A launch is identified by the `--` separator immediately after its name.
/// This lets a process be named after an operation, for example
/// `park status -- ./server`.
pub fn parse_invocation<I, T>(args: I) -> Result<Invocation, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();

    if args.get(2).is_some_and(|arg| arg == "--") {
        let parsed = LaunchCli::try_parse_from(args)?;
        return Ok(Invocation::Launch {
            name: parsed.name,
            command: parsed.command,
        });
    }

    if args.get(1).is_some_and(|arg| arg == "run") {
        let mut run_args = args;
        run_args.remove(1);
        let parsed = RunCli::try_parse_from(run_args)?;
        return Ok(Invocation::Launch {
            name: parsed.name,
            command: parsed.command,
        });
    }

    let mut operation_args = args;
    normalize_operation_alias(&mut operation_args);
    let parsed = OperationCli::try_parse_from(operation_args)?;
    Ok(Invocation::Operation(parsed.operation.into()))
}

fn normalize_operation_alias(args: &mut [OsString]) {
    let Some(command) = args.get_mut(1) else {
        return;
    };

    let alias = match command.to_str() {
        Some("--ps") => "ps",
        Some("--status") => "status",
        Some("--logs") => "logs",
        Some("--stop") => "stop",
        Some("--restart") => "restart",
        Some("--start") => "start",
        Some("--signal") => "signal",
        Some("--rm") => "rm",
        Some("--clean") => "clean",
        Some("--wait") => "wait",
        _ => return,
    };
    *command = OsString::from(alias);
}

impl From<OperationCliCommand> for Operation {
    fn from(command: OperationCliCommand) -> Self {
        match command {
            OperationCliCommand::Ps { json } => Self::Ps { json },
            OperationCliCommand::Status { name, json } => Self::Status { name, json },
            OperationCliCommand::Logs(args) => Self::Logs(LogsArgs {
                name: args.name,
                tail: args.tail,
                head: args.head,
                follow: args.follow,
                grep: args.grep,
                stdout: args.stdout,
                stderr: args.stderr,
                json: args.json,
            }),
            OperationCliCommand::Stop { name, force } => Self::Stop { name, force },
            OperationCliCommand::Restart { name } => Self::Restart { name },
            OperationCliCommand::Start { name } => Self::Start { name },
            OperationCliCommand::Signal { name, signal } => Self::Signal { name, signal },
            OperationCliCommand::Rm { name, keep_logs } => Self::Rm { name, keep_logs },
            OperationCliCommand::Clean => Self::Clean,
            OperationCliCommand::Wait(args) => Self::Wait(WaitArgs {
                name: args.name,
                state: args.state,
                match_text: args.match_text,
                exit: args.exit,
                timeout: args.timeout,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultStatus {
    Success,
    Failure,
    MissingRecord,
    DuplicateRecord,
    InvalidState,
}

impl ResultStatus {
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::Failure => 1,
            Self::MissingRecord => 3,
            Self::DuplicateRecord => 4,
            Self::InvalidState => 5,
        }
    }

    pub const fn is_success(self) -> bool {
        matches!(self, Self::Success)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResultError {
    pub code: ResultStatus,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandResult<T> {
    pub status: ResultStatus,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ResultError>,
}

impl<T> CommandResult<T> {
    pub fn success(data: Option<T>, message: Option<String>) -> Self {
        Self {
            status: ResultStatus::Success,
            ok: true,
            message,
            data,
            error: None,
        }
    }

    pub fn error(status: ResultStatus, message: impl Into<String>) -> Self {
        assert!(!status.is_success());
        let message = message.into();
        Self {
            status,
            ok: false,
            message: None,
            data: None,
            error: Some(ResultError {
                code: status,
                message,
            }),
        }
    }

    pub fn human_message(&self) -> &str {
        self.error
            .as_ref()
            .map(|error| error.message.as_str())
            .or(self.message.as_deref())
            .unwrap_or("ok")
    }
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("could not render JSON result: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn render_json<T: Serialize>(result: &CommandResult<T>) -> Result<String, RenderError> {
    Ok(serde_json::to_string(result)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(args: &[&str]) -> Invocation {
        parse_invocation(args.iter().copied()).expect("arguments should parse")
    }

    #[test]
    fn parses_short_launch_and_preserves_command_arguments() {
        assert_eq!(
            parse(&["park", "dev", "--", "cargo", "run", "--release"]),
            Invocation::Launch {
                name: "dev".into(),
                command: vec!["cargo".into(), "run".into(), "--release".into()],
            }
        );
    }

    #[test]
    fn parses_operation_word_as_a_name_when_launch_separator_follows() {
        assert_eq!(
            parse(&["park", "status", "--", "./server"]),
            Invocation::Launch {
                name: "status".into(),
                command: vec!["./server".into()],
            }
        );
    }

    #[test]
    fn parses_command_arguments_that_begin_with_a_dash() {
        assert_eq!(
            parse(&["park", "dev", "--", "-custom-command", "--flag"]),
            Invocation::Launch {
                name: "dev".into(),
                command: vec!["-custom-command".into(), "--flag".into()],
            }
        );
    }

    #[test]
    fn parses_a_dash_prefixed_name() {
        assert_eq!(
            parse(&["park", "-status", "--", "./server"]),
            Invocation::Launch {
                name: "-status".into(),
                command: vec!["./server".into()],
            }
        );
    }

    #[test]
    fn parses_status_and_json() {
        assert_eq!(
            parse(&["park", "status", "dev", "--json"]),
            Invocation::Operation(Operation::Status {
                name: "dev".into(),
                json: true,
            })
        );
    }

    #[test]
    fn parses_long_operation_alias() {
        assert_eq!(
            parse(&["park", "--status", "dev"]),
            Invocation::Operation(Operation::Status {
                name: "dev".into(),
                json: false,
            })
        );
    }

    #[test]
    fn distinguishes_a_long_operation_alias_from_a_dash_prefixed_name() {
        assert_eq!(
            parse(&["park", "--status", "--", "./server"]),
            Invocation::Launch {
                name: "--status".into(),
                command: vec!["./server".into()],
            }
        );
    }

    #[test]
    fn parses_explicit_run_alias() {
        assert_eq!(
            parse(&["park", "run", "dev", "--", "cargo", "run"]),
            Invocation::Launch {
                name: "dev".into(),
                command: vec!["cargo".into(), "run".into()],
            }
        );
    }

    #[test]
    fn serializes_success_result_schema() {
        let result = CommandResult::success(Some(json!({"name": "dev", "state": "running"})), None);
        assert_eq!(
            render_json(&result).expect("result should serialize"),
            r#"{"status":"success","ok":true,"data":{"name":"dev","state":"running"}}"#
        );
    }

    #[test]
    fn serializes_error_result_schema() {
        let result = CommandResult::<()>::error(ResultStatus::MissingRecord, "no such process");
        assert_eq!(
            render_json(&result).expect("result should serialize"),
            r#"{"status":"missing_record","ok":false,"error":{"code":"missing_record","message":"no such process"}}"#
        );
    }
}
