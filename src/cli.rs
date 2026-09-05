use std::ffi::OsString;

use clap::{ArgAction, ArgGroup, Args, CommandFactory, Parser, Subcommand};
use std::num::ParseIntError;

use crate::lifecycle::ProcessState;
use crate::process::validate_process_name;

/// The operation requested by a user, before a daemon handles it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    Launch {
        name: OsString,
        env_files: Vec<OsString>,
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
    Restart(RestartArgs),
    Start(StartArgs),
    Signal { name: OsString, signal: String },
    Rm { name: OsString, keep_logs: bool },
    Clean,
    Wait(WaitArgs),
    Env(EnvArgs),
    Daemon(DaemonOperation),
    Help,
    HelpSkills { json: bool },
}

impl Operation {
    fn requests_json(&self) -> bool {
        match self {
            Self::Ps { json } | Self::Status { json, .. } => *json,
            Self::Logs(args) => args.json,
            Self::Env(args) => args.json,
            Self::Daemon(operation) => operation.requests_json(),
            Self::HelpSkills { json } => *json,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonOperation {
    Status { json: bool },
    Reexec { force: bool },
    Config { json: bool },
}

impl DaemonOperation {
    fn requests_json(&self) -> bool {
        matches!(
            self,
            Self::Status { json: true } | Self::Config { json: true }
        )
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
    pub state: Option<ProcessState>,
    pub match_text: Option<String>,
    pub exit: bool,
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestartArgs {
    pub name: OsString,
    pub recapture_env: bool,
    pub env_files: Vec<OsString>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartArgs {
    pub name: OsString,
    pub env_files: Vec<OsString>,
    pub command: Vec<OsString>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvArgs {
    pub name: OsString,
    pub set: Vec<OsString>,
    pub unset: Vec<OsString>,
    pub json: bool,
}

#[derive(Debug, Parser)]
#[command(
    name = "park",
    version,
    about = "Project-scoped local process manager",
    after_help = "Launch a command with:\n  park <NAME> -- <COMMAND> [ARGUMENTS...]\n\nThe explicit equivalent is:\n  park run <NAME> -- <COMMAND> [ARGUMENTS...]\n\nRecords are scoped to the current project directory. Use `park help --skill`\nfor AI-agent integration instructions."
)]
struct LaunchCli {
    #[arg(
        value_name = "NAME",
        allow_hyphen_values = true,
        help = "Name for the retained process record"
    )]
    name: OsString,

    #[arg(long = "env-file", value_name = "PATH", action = ArgAction::Append, help = "Read this dotenv file when spawning")]
    env_files: Vec<OsString>,

    #[arg(
        last = true,
        required = true,
        value_name = "COMMAND",
        help = "Command and arguments after `--`"
    )]
    command: Vec<OsString>,
}

#[derive(Debug, Parser)]
#[command(
    name = "park run",
    version,
    about = "Start a named command",
    after_help = "The command begins after the `--` separator and is retained for\nfuture lifecycle operations."
)]
struct RunCli {
    #[arg(
        value_name = "NAME",
        allow_hyphen_values = true,
        help = "Name for the retained process record"
    )]
    name: OsString,

    #[arg(long = "env-file", value_name = "PATH", action = ArgAction::Append, help = "Read this dotenv file when spawning")]
    env_files: Vec<OsString>,

    #[arg(
        last = true,
        required = true,
        value_name = "COMMAND",
        help = "Command and arguments after `--`"
    )]
    command: Vec<OsString>,
}

#[derive(Debug, Parser)]
#[command(
    name = "park",
    version,
    about = "Project-scoped local process manager",
    disable_help_subcommand = true,
    after_help = "The launch form is `park <NAME> -- <COMMAND> [ARGUMENTS...]`.\n\nUse `park help --skill` for AI-agent integration instructions."
)]
struct OperationCli {
    #[command(subcommand)]
    operation: OperationCliCommand,
}

pub(crate) fn command_help() -> String {
    OperationCli::command().render_long_help().to_string()
}

#[derive(Debug, Subcommand)]
enum OperationCliCommand {
    #[command(name = "ps", about = "List records in the current project")]
    Ps {
        #[arg(long, help = "Render machine-readable JSON")]
        json: bool,
    },
    #[command(name = "status", about = "Show the status of one record")]
    Status {
        #[arg(value_name = "NAME", allow_hyphen_values = true, help = "Record name")]
        name: OsString,
        #[arg(long, help = "Render machine-readable JSON")]
        json: bool,
    },
    #[command(
        name = "logs",
        about = "Read retained process output",
        after_help = "Without --stdout or --stderr, output is stdout followed by stderr.\n--grep uses a literal substring and filtering happens before --head or --tail.\nWith --follow, new output is streamed as it is appended."
    )]
    Logs(LogsCliArgs),
    #[command(
        name = "stop",
        about = "Stop a managed process group",
        after_help = "Sends SIGTERM and escalates to SIGKILL after the grace period.\nUse --force to send SIGKILL immediately."
    )]
    Stop {
        #[arg(value_name = "NAME", allow_hyphen_values = true, help = "Record name")]
        name: OsString,
        #[arg(long, help = "Send SIGKILL immediately")]
        force: bool,
    },
    #[command(
        name = "restart",
        about = "Restart a retained process record",
        after_help = "Restarts the recorded command in its project and appends new\noutput to the existing stdout and stderr logs."
    )]
    Restart {
        #[arg(value_name = "NAME", allow_hyphen_values = true, help = "Record name")]
        name: OsString,
        #[arg(long, help = "Replace the stored client environment capture")]
        recapture_env: bool,
        #[arg(long = "env-file", value_name = "PATH", requires = "recapture_env", action = ArgAction::Append, help = "Replace the stored dotenv file list")]
        env_files: Vec<OsString>,
    },
    #[command(
        name = "start",
        about = "Start a retained terminal record",
        after_help = "Starts a stopped terminal record using its recorded command.\nUse the launch form to create a new record."
    )]
    Start {
        #[arg(value_name = "NAME", allow_hyphen_values = true, help = "Record name")]
        name: OsString,
        #[arg(long = "env-file", value_name = "PATH", action = ArgAction::Append, help = "Read this dotenv file when creating a record")]
        env_files: Vec<OsString>,
        #[arg(
            last = true,
            value_name = "COMMAND",
            help = "Create a record with this command after `--`"
        )]
        command: Vec<OsString>,
    },
    #[command(
        name = "signal",
        about = "Send a signal to a managed process group",
        after_help = "Supported names: HUP, INT, QUIT, TERM, USR1, USR2, STOP,\nCONT, and KILL, with an optional SIG prefix."
    )]
    Signal {
        #[arg(value_name = "NAME", allow_hyphen_values = true, help = "Record name")]
        name: OsString,
        #[arg(
            value_name = "SIGNAL",
            help = "Signal name, with or without the SIG prefix"
        )]
        signal: String,
    },
    #[command(
        name = "rm",
        about = "Remove a retained process record",
        after_help = "Active records cannot be removed. Logs are removed unless\n--keep-logs is supplied."
    )]
    Rm {
        #[arg(value_name = "NAME", allow_hyphen_values = true, help = "Record name")]
        name: OsString,
        #[arg(long, help = "Keep the record's stdout and stderr logs")]
        keep_logs: bool,
    },
    #[command(
        name = "clean",
        about = "Remove eligible terminal records",
        after_help = "Removes terminal records whose managed process group is gone.\nActive records are never removed."
    )]
    Clean,
    #[command(
        name = "wait",
        about = "Wait for a record state or output",
        after_help = "Choose exactly one of --state, --match, or --exit.\n--timeout accepts a non-negative duration ending in ms, s, or m."
    )]
    Wait(WaitCliArgs),
    #[command(
        name = "env",
        about = "Inspect or update a record environment",
        after_help = "Environment values are shown only by this command and apply to future starts."
    )]
    Env {
        #[arg(value_name = "NAME", allow_hyphen_values = true, help = "Record name")]
        name: OsString,
        #[arg(long = "set", value_name = "KEY=VALUE", action = ArgAction::Append, help = "Set an explicit environment value")]
        set: Vec<OsString>,
        #[arg(long = "unset", value_name = "KEY", action = ArgAction::Append, help = "Remove an explicit environment value")]
        unset: Vec<OsString>,
        #[arg(long, help = "Render machine-readable JSON")]
        json: bool,
    },
    #[command(
        name = "daemon",
        about = "Manage the per-user Park daemon",
        after_help = "Daemon commands use the per-user runtime and state directories."
    )]
    Daemon {
        #[command(subcommand)]
        operation: DaemonCliCommand,
    },
    #[command(
        name = "help",
        about = "Show command help or integration guidance",
        after_help = "Without an option, prints the same command overview as park --help.\nUse --skill for AI-agent integration instructions."
    )]
    Help {
        #[arg(
            long = "skill",
            visible_alias = "skills",
            help = "Show AI-agent integration instructions"
        )]
        skills: bool,
        #[arg(long, requires = "skills", help = "Render machine-readable JSON")]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum DaemonCliCommand {
    #[command(name = "status", about = "Show daemon status")]
    Status {
        #[arg(long, help = "Render machine-readable JSON")]
        json: bool,
    },
    #[command(
        name = "reexec",
        about = "Request a daemon re-exec",
        after_help = "Daemon re-exec is reserved for the handoff workflow and is\ncurrently not implemented."
    )]
    Reexec {
        #[arg(long, help = "Force the re-exec request")]
        force: bool,
    },
    #[command(name = "config", about = "Show effective daemon configuration")]
    Config {
        #[arg(long, help = "Render machine-readable JSON")]
        json: bool,
    },
}

#[derive(Debug, Args)]
struct LogsCliArgs {
    #[arg(value_name = "NAME", allow_hyphen_values = true, help = "Record name")]
    name: OsString,
    #[arg(
        long,
        value_name = "N",
        conflicts_with = "head",
        help = "Show at most N retained lines"
    )]
    tail: Option<u64>,
    #[arg(
        long,
        value_name = "N",
        conflicts_with = "tail",
        help = "Show the first N retained lines"
    )]
    head: Option<u64>,
    #[arg(long, help = "Continue streaming output as it is appended")]
    follow: bool,
    #[arg(
        long = "grep",
        value_name = "PATTERN",
        help = "Keep lines containing this literal substring"
    )]
    grep: Option<String>,
    #[arg(long, conflicts_with = "stderr", help = "Show stdout only")]
    stdout: bool,
    #[arg(long, conflicts_with = "stdout", help = "Show stderr only")]
    stderr: bool,
    #[arg(long, help = "Render machine-readable JSON")]
    json: bool,
}

#[derive(Debug, Args)]
#[command(group(ArgGroup::new("wait-condition").required(true)))]
struct WaitCliArgs {
    #[arg(value_name = "NAME", allow_hyphen_values = true, help = "Record name")]
    name: OsString,
    #[arg(
        long,
        value_name = "STATE",
        group = "wait-condition",
        value_parser = parse_state,
        help = "Wait until the record reaches this state"
    )]
    state: Option<ProcessState>,
    #[arg(
        long = "match",
        value_name = "TEXT",
        group = "wait-condition",
        help = "Wait for this literal text in retained output"
    )]
    match_text: Option<String>,
    #[arg(long, group = "wait-condition", help = "Wait until the record exits")]
    exit: bool,
    #[arg(
        long,
        value_name = "DURATION",
        value_parser = parse_duration,
        help = "Fail after a duration ending in ms, s, or m"
    )]
    timeout: Option<u64>,
}

fn parse_state(value: &str) -> Result<ProcessState, String> {
    value.parse()
}

fn parse_duration(value: &str) -> Result<u64, String> {
    let (number, multiplier) = if let Some(value) = value.strip_suffix("ms") {
        (value, 1_u64)
    } else if let Some(value) = value.strip_suffix('s') {
        (value, 1_000)
    } else if let Some(value) = value.strip_suffix('m') {
        (value, 60_000)
    } else {
        return Err("invalid duration; use a non-negative value ending in ms, s, or m".to_owned());
    };
    let number = number
        .parse::<u64>()
        .map_err(|error: ParseIntError| error.to_string())?;
    number
        .checked_mul(multiplier)
        .ok_or_else(|| "duration is too large".to_owned())
}

/// Parse the public CLI grammar while validating names for new launches.
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

    if args.get(1).is_none_or(|arg| arg != "run")
        && args
            .get(2)
            .is_some_and(|arg| arg == "--" || arg == "--env-file")
    {
        let parsed = LaunchCli::try_parse_from(args)?;
        validate_launch_name(&parsed.name)?;
        return Ok(Invocation::Launch {
            name: parsed.name,
            env_files: parsed.env_files,
            command: parsed.command,
        });
    }

    if args.get(1).is_some_and(|arg| arg == "run") {
        let mut run_args = args;
        run_args.remove(1);
        let parsed = RunCli::try_parse_from(run_args)?;
        validate_launch_name(&parsed.name)?;
        return Ok(Invocation::Launch {
            name: parsed.name,
            env_files: parsed.env_files,
            command: parsed.command,
        });
    }

    let mut operation_args = args;
    normalize_operation_alias(&mut operation_args);
    let parsed = OperationCli::try_parse_from(operation_args)?;
    let operation = parsed.operation.into();
    validate_operation_name(&operation)?;
    if let Operation::Start(args) = &operation
        && args.command.is_empty()
        && !args.env_files.is_empty()
    {
        return Err(clap::Error::raw(
            clap::error::ErrorKind::MissingRequiredArgument,
            "--env-file requires a command when creating a new record",
        ));
    }
    Ok(Invocation::Operation(operation))
}

fn validate_launch_name(name: &OsString) -> Result<(), clap::Error> {
    validate_process_name(name)
        .map_err(|error| clap::Error::raw(clap::error::ErrorKind::InvalidValue, error.to_string()))
}

fn validate_operation_name(operation: &Operation) -> Result<(), clap::Error> {
    let name = match operation {
        Operation::Status { name, .. }
        | Operation::Logs(LogsArgs { name, .. })
        | Operation::Stop { name, .. }
        | Operation::Restart(RestartArgs { name, .. })
        | Operation::Start(StartArgs { name, .. })
        | Operation::Signal { name, .. }
        | Operation::Rm { name, .. }
        | Operation::Wait(WaitArgs { name, .. })
        | Operation::Env(EnvArgs { name, .. }) => Some(name),
        Operation::Ps { .. }
        | Operation::Clean
        | Operation::Daemon(_)
        | Operation::Help
        | Operation::HelpSkills { .. } => None,
    };
    if let Some(name) = name {
        validate_launch_name(name)?;
    }
    Ok(())
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
            OperationCliCommand::Restart {
                name,
                recapture_env,
                env_files,
            } => Self::Restart(RestartArgs {
                name,
                recapture_env,
                env_files,
            }),
            OperationCliCommand::Start {
                name,
                env_files,
                command,
            } => Self::Start(StartArgs {
                name,
                env_files,
                command,
            }),
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
            OperationCliCommand::Env {
                name,
                set,
                unset,
                json,
            } => Self::Env(EnvArgs {
                name,
                set,
                unset,
                json,
            }),
            OperationCliCommand::Daemon { operation } => Self::Daemon(match operation {
                DaemonCliCommand::Status { json } => DaemonOperation::Status { json },
                DaemonCliCommand::Reexec { force } => DaemonOperation::Reexec { force },
                DaemonCliCommand::Config { json } => DaemonOperation::Config { json },
            }),
            OperationCliCommand::Help { skills, json } => {
                if skills {
                    Self::HelpSkills { json }
                } else {
                    Self::Help
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
