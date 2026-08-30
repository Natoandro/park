use std::{env, process};

use park_cli::{
    CommandResult, INTERNAL_DAEMON_ARGUMENT, INTERNAL_SUPERVISOR_ARGUMENT, Invocation, Operation,
    ResultStatus, StoragePaths, parse_invocation, render_json, request_for_launch, request_for_ps,
    request_for_status, request_with_daemon_start, resolve_current_project, run_daemon,
};
use serde_json::Value;

fn main() {
    if env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == INTERNAL_SUPERVISOR_ARGUMENT)
    {
        run_supervisor();
    }
    if env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == INTERNAL_DAEMON_ARGUMENT)
    {
        let paths = StoragePaths::from_process_environment().unwrap_or_else(|error| {
            eprintln!("error: {error}");
            process::exit(ResultStatus::Failure.exit_code().into());
        });
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("daemon runtime should be created");
        match runtime.block_on(run_daemon(paths)) {
            Ok(_) => process::exit(0),
            Err(error) => {
                eprintln!("error: {error}");
                process::exit(ResultStatus::Failure.exit_code().into());
            }
        }
    }

    let invocation = match parse_invocation(env::args_os()) {
        Ok(invocation) => invocation,
        Err(error) => error.exit(),
    };

    let requests_json = invocation.requests_json();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("client runtime should be created");
    let result = runtime.block_on(execute(invocation));

    if requests_json {
        println!(
            "{}",
            render_json(&result).expect("the result schema must be serializable")
        );
    } else if result.ok {
        if let Some(data) = &result.data {
            println!(
                "{}",
                serde_json::to_string_pretty(data).expect("response data should serialize")
            );
        } else {
            println!("{}", result.human_message());
        }
    } else {
        eprintln!("error: {}", result.human_message());
    }
    process::exit(result.status.exit_code().into());
}

#[cfg(target_os = "linux")]
fn run_supervisor() -> ! {
    use nix::sys::prctl;
    use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction};
    use nix::unistd::getppid;

    let mut arguments = env::args_os().skip(2);
    let expected_parent = arguments
        .next()
        .and_then(|value| value.into_string().ok())
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|pid| *pid > 0)
        .unwrap_or_else(|| supervisor_usage_error("missing valid parent process ID"));
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--")) {
        supervisor_usage_error("missing command separator");
    }
    let executable = arguments
        .next()
        .unwrap_or_else(|| supervisor_usage_error("missing command"));

    let action = SigAction::new(
        SigHandler::Handler(kill_managed_group),
        SaFlags::SA_RESTART,
        SigSet::empty(),
    );
    unsafe {
        sigaction(Signal::SIGTERM, &action).unwrap_or_else(|error| {
            supervisor_usage_error(&format!("could not handle parent death: {error}"))
        });
    }
    prctl::set_pdeathsig(Signal::SIGTERM).unwrap_or_else(|error| {
        supervisor_usage_error(&format!("could not monitor parent death: {error}"))
    });
    if getppid().as_raw() != expected_parent {
        kill_managed_group(Signal::SIGTERM as i32);
    }

    match process::Command::new(executable).args(arguments).status() {
        Ok(status) => process::exit(status.code().unwrap_or(1)),
        Err(error) => {
            eprintln!("error: could not spawn managed command: {error}");
            process::exit(127);
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn run_supervisor() -> ! {
    eprintln!("error: managed process supervision requires Linux");
    process::exit(ResultStatus::Failure.exit_code().into());
}

#[cfg(target_os = "linux")]
extern "C" fn kill_managed_group(_: i32) {
    unsafe {
        nix::libc::kill(0, nix::libc::SIGKILL);
    }
}

fn supervisor_usage_error(message: &str) -> ! {
    eprintln!("error: invalid internal supervisor invocation: {message}");
    process::exit(ResultStatus::Failure.exit_code().into());
}

async fn execute(invocation: Invocation) -> CommandResult<Value> {
    let paths = match StoragePaths::from_process_environment() {
        Ok(paths) => paths,
        Err(error) => return CommandResult::error(ResultStatus::Failure, error.to_string()),
    };
    let project = match resolve_current_project() {
        Ok(project) => project,
        Err(error) => return CommandResult::error(ResultStatus::Failure, error.to_string()),
    };

    let request = match invocation {
        Invocation::Launch { name, command } => request_for_launch(1, project, name, command),
        Invocation::Operation(Operation::Ps { .. }) => request_for_ps(1, project),
        Invocation::Operation(Operation::Status { name, .. }) => {
            request_for_status(1, park_cli::ProcessKey::new(project, name))
        }
        Invocation::Operation(other) => {
            return CommandResult::error(
                ResultStatus::Failure,
                format!("operation {other:?} is not implemented yet"),
            );
        }
    };

    match request_with_daemon_start(&paths, &request).await {
        Ok(response) => response.result,
        Err(error) => CommandResult::error(ResultStatus::Failure, error.to_string()),
    }
}
