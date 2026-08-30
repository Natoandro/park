use std::ffi::OsString;

use clap::{ArgGroup, Args, Parser, Subcommand};
use std::num::ParseIntError;

use crate::lifecycle::ProcessState;

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
    HelpSkills { json: bool },
}

impl Operation {
    fn requests_json(&self) -> bool {
        match self {
            Self::Ps { json } | Self::Status { json, .. } => *json,
            Self::Logs(args) => args.json,
            Self::HelpSkills { json } => *json,
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
    pub state: Option<ProcessState>,
    pub match_text: Option<String>,
    pub exit: bool,
    pub timeout: Option<u64>,
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
#[command(
    name = "park",
    version,
    about = "Project-scoped local process manager",
    disable_help_subcommand = true
)]
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
    #[command(name = "help")]
    Help {
        #[arg(long, required = true)]
        skills: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Args)]
struct LogsCliArgs {
    #[arg(value_name = "NAME", allow_hyphen_values = true)]
    name: OsString,
    #[arg(long, value_name = "N", conflicts_with = "head")]
    tail: Option<u64>,
    #[arg(long, value_name = "N", conflicts_with = "tail")]
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
#[command(group(ArgGroup::new("wait-condition").required(true)))]
struct WaitCliArgs {
    #[arg(value_name = "NAME", allow_hyphen_values = true)]
    name: OsString,
    #[arg(long, value_name = "STATE", group = "wait-condition", value_parser = parse_state)]
    state: Option<ProcessState>,
    #[arg(long = "match", value_name = "TEXT", group = "wait-condition")]
    match_text: Option<String>,
    #[arg(long, group = "wait-condition")]
    exit: bool,
    #[arg(long, value_name = "DURATION", value_parser = parse_duration)]
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
            OperationCliCommand::Help { json, .. } => Self::HelpSkills { json },
        }
    }
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
