use std::{env, io::Write, process};

use park_cli::{
    CommandResult, INTERNAL_DAEMON_ARGUMENT, INTERNAL_SUPERVISOR_ARGUMENT, Invocation,
    IpcLogOptions, Operation, ResultStatus, StoragePaths, command_help_result, parse_invocation,
    render_json, request_for_clean, request_for_daemon_config, request_for_daemon_status,
    request_for_launch, request_for_logs, request_for_ps, request_for_remove, request_for_restart,
    request_for_signal, request_for_start, request_for_status, request_for_stop, request_for_wait,
    request_with_daemon_start, resolve_current_project, run_daemon, skills_help_result,
    stream_request_with_daemon_start,
};
use serde_json::Value;

mod render;

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
    let follows_logs = matches!(
        &invocation,
        Invocation::Operation(Operation::Logs(args)) if args.follow
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("client runtime should be created");
    let mut follow_output = |chunk: &str| {
        let mut stdout = std::io::stdout().lock();
        let _ = stdout.write_all(chunk.as_bytes());
        let _ = stdout.flush();
    };
    let mut result = runtime.block_on(execute(invocation, &mut follow_output));
    render::decode_json_result(&mut result);

    if requests_json {
        println!(
            "{}",
            render_json(&result).expect("the result schema must be serializable")
        );
    } else if follows_logs && result.ok {
        // Follow output is written as each IPC frame arrives.
    } else if result.ok {
        print!("{}", render::human_result(&result));
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
        sigaction(Signal::SIGURG, &action).unwrap_or_else(|error| {
            supervisor_usage_error(&format!("could not handle parent death: {error}"))
        });
    }
    prctl::set_pdeathsig(Signal::SIGURG).unwrap_or_else(|error| {
        supervisor_usage_error(&format!("could not monitor parent death: {error}"))
    });
    if getppid().as_raw() != expected_parent {
        kill_managed_group(Signal::SIGURG as i32);
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

async fn execute(invocation: Invocation, on_follow: &mut dyn FnMut(&str)) -> CommandResult<Value> {
    if let Invocation::Operation(Operation::Help) = &invocation {
        return command_help_result();
    }
    if let Invocation::Operation(Operation::HelpSkills { json }) = &invocation {
        return skills_help_result(*json);
    }
    if let Invocation::Operation(Operation::Daemon(operation)) = &invocation {
        let paths = match StoragePaths::from_process_environment() {
            Ok(paths) => paths,
            Err(error) => return CommandResult::error(ResultStatus::Failure, error.to_string()),
        };
        let request = match operation {
            park_cli::DaemonOperation::Status { .. } => request_for_daemon_status(1),
            park_cli::DaemonOperation::Config { .. } => request_for_daemon_config(1),
            park_cli::DaemonOperation::Reexec { .. } => {
                return CommandResult::error(
                    ResultStatus::Failure,
                    "daemon reexec is not implemented",
                );
            }
        };
        return match request_with_daemon_start(&paths, &request).await {
            Ok(response) => response.result,
            Err(error) => CommandResult::error(ResultStatus::Failure, error.to_string()),
        };
    }

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
        Invocation::Operation(Operation::Logs(args)) => {
            let stream = if args.stdout {
                "stdout"
            } else if args.stderr {
                "stderr"
            } else {
                "combined"
            };
            let mut content = String::new();
            let request = request_for_logs(
                1,
                park_cli::ProcessKey::new(project, args.name.clone()),
                IpcLogOptions {
                    tail: args.tail,
                    head: args.head,
                    follow: args.follow,
                    grep: args.grep.clone(),
                    stdout: args.stdout,
                    stderr: args.stderr,
                },
            );
            let response = match stream_request_with_daemon_start(&paths, &request, |chunk| {
                content.push_str(chunk);
                if args.follow && !args.json {
                    on_follow(chunk);
                }
            })
            .await
            {
                Ok(response) => response,
                Err(error) => {
                    return CommandResult::error(ResultStatus::Failure, error.to_string());
                }
            };
            if !response.result.ok {
                return response.result;
            }
            let state = response
                .result
                .data
                .as_ref()
                .and_then(|data| data.get("state"))
                .cloned()
                .unwrap_or(Value::Null);
            return CommandResult::success(
                Some(serde_json::json!({
                    "stream": stream,
                    "content": content,
                    "state": state,
                })),
                None,
            );
        }
        Invocation::Operation(Operation::Stop { name, force }) => {
            request_for_stop(1, park_cli::ProcessKey::new(project, name), force)
        }
        Invocation::Operation(Operation::Signal { name, signal }) => {
            request_for_signal(1, park_cli::ProcessKey::new(project, name), signal)
        }
        Invocation::Operation(Operation::Restart { name }) => {
            request_for_restart(1, park_cli::ProcessKey::new(project, name))
        }
        Invocation::Operation(Operation::Start { name }) => {
            request_for_start(1, park_cli::ProcessKey::new(project, name))
        }
        Invocation::Operation(Operation::Rm { name, keep_logs }) => {
            request_for_remove(1, park_cli::ProcessKey::new(project, name), keep_logs)
        }
        Invocation::Operation(Operation::Clean) => request_for_clean(1),
        Invocation::Operation(Operation::Wait(args)) => {
            let request = request_for_wait(
                1,
                park_cli::ProcessKey::new(project, args.name),
                args.state,
                args.match_text,
                args.exit,
                args.timeout,
            );
            let response = match stream_request_with_daemon_start(&paths, &request, |_| {}).await {
                Ok(response) => response,
                Err(error) => {
                    return CommandResult::error(ResultStatus::Failure, error.to_string());
                }
            };
            if !response.result.ok {
                return response.result;
            }
            let Some(data) = response.result.data else {
                return CommandResult::error(
                    ResultStatus::Failure,
                    "wait response is missing data",
                );
            };
            return CommandResult::success(data.get("record").cloned(), None);
        }
        Invocation::Operation(Operation::Help | Operation::HelpSkills { .. }) => {
            unreachable!("skills help is handled before daemon setup")
        }
        Invocation::Operation(Operation::Daemon(_)) => {
            unreachable!("daemon-management commands are handled before daemon setup")
        }
    };

    match request_with_daemon_start(&paths, &request).await {
        Ok(response) => response.result,
        Err(error) => CommandResult::error(ResultStatus::Failure, error.to_string()),
    }
}
