use std::{env, process};

use park_cli::{
    CommandResult, INTERNAL_DAEMON_ARGUMENT, Invocation, Operation, ResultStatus, StoragePaths,
    parse_invocation, render_json, request_for_ps, request_for_status, request_with_daemon_start,
    resolve_current_project, run_daemon,
};
use serde_json::Value;

fn main() {
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

async fn execute(invocation: Invocation) -> CommandResult<Value> {
    let operation = match invocation {
        Invocation::Operation(operation) => operation,
        Invocation::Launch { .. } => {
            return CommandResult::error(
                ResultStatus::Failure,
                "launching processes is not implemented yet",
            );
        }
    };

    let paths = match StoragePaths::from_process_environment() {
        Ok(paths) => paths,
        Err(error) => return CommandResult::error(ResultStatus::Failure, error.to_string()),
    };
    let project = match resolve_current_project() {
        Ok(project) => project,
        Err(error) => return CommandResult::error(ResultStatus::Failure, error.to_string()),
    };

    let request = match operation {
        Operation::Ps { .. } => request_for_ps(1, project),
        Operation::Status { name, .. } => {
            request_for_status(1, park_cli::ProcessKey::new(project, name))
        }
        other => {
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
