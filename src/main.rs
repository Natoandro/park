use std::{env, process};

use park_cli::{CommandResult, ResultStatus, parse_invocation, render_json};

fn main() {
    let invocation = match parse_invocation(env::args_os()) {
        Ok(invocation) => invocation,
        Err(error) => error.exit(),
    };

    let result = CommandResult::<()>::error(
        ResultStatus::Failure,
        format!(
            "phase 1 parser scaffold does not execute {:?} yet",
            invocation
        ),
    );

    if invocation.requests_json() {
        println!(
            "{}",
            render_json(&result).expect("the result schema must be serializable")
        );
    } else {
        eprintln!("error: {}", result.human_message());
    }
    process::exit(result.status.exit_code().into());
}
