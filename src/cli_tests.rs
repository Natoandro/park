use super::*;

fn parse(args: &[&str]) -> Invocation {
    parse_invocation(args.iter().copied()).expect("arguments should parse")
}

#[test]
fn parses_short_launch_and_preserves_command_arguments() {
    assert_eq!(
        parse(&["park", "dev", "--", "cargo", "run", "--release"]),
        Invocation::Launch {
            name: "dev".into(),
            env_files: vec![],
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
            env_files: vec![],
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
            env_files: vec![],
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
            env_files: vec![],
            command: vec!["./server".into()],
        }
    );
}

#[test]
fn accepts_colons_in_launch_names() {
    assert_eq!(
        parse(&["park", "api:dev", "--", "./server"]),
        Invocation::Launch {
            name: "api:dev".into(),
            env_files: vec![],
            command: vec!["./server".into()],
        }
    );
}

#[test]
fn rejects_non_ascii_or_whitespace_in_launch_names() {
    assert!(parse_invocation(["park", "api dev", "--", "./server"]).is_err());
    assert!(parse_invocation(["park", "api\u{e9}", "--", "./server"]).is_err());
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
fn parses_process_listing_scopes() {
    assert_eq!(
        parse(&["park", "ps"]),
        Invocation::Operation(Operation::Ps {
            scope: PsScope::Current,
            json: false,
        })
    );
    assert_eq!(
        parse(&["park", "ps", "--scope", "subtree", "--json"]),
        Invocation::Operation(Operation::Ps {
            scope: PsScope::Subtree,
            json: true,
        })
    );
    assert_eq!(
        parse(&["park", "ps", "--scope", "global"]),
        Invocation::Operation(Operation::Ps {
            scope: PsScope::Global,
            json: false,
        })
    );
    assert!(parse_invocation(["park", "ps", "--scope", "invalid"]).is_err());
}

#[test]
fn parses_daemon_management_commands() {
    assert_eq!(
        parse(&["park", "daemon", "status", "--json"]),
        Invocation::Operation(Operation::Daemon(DaemonOperation::Status { json: true }))
    );
    assert_eq!(
        parse(&["park", "daemon", "reexec", "--force"]),
        Invocation::Operation(Operation::Daemon(DaemonOperation::Reexec { force: true }))
    );
    assert_eq!(
        parse(&["park", "daemon", "config"]),
        Invocation::Operation(Operation::Daemon(DaemonOperation::Config { json: false }))
    );
}

#[test]
fn daemon_management_json_is_limited_to_inspection_commands() {
    assert!(parse(&["park", "daemon", "status", "--json"]).requests_json());
    assert!(parse(&["park", "daemon", "config", "--json"]).requests_json());
    assert!(!parse(&["park", "daemon", "reexec"]).requests_json());
    assert!(parse_invocation(["park", "daemon", "reexec", "--json"]).is_err());
}

#[test]
fn requires_a_daemon_management_subcommand() {
    assert!(parse_invocation(["park", "daemon"] as [&str; 2]).is_err());
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
fn parses_skills_help() {
    assert_eq!(
        parse(&["park", "help", "--skill"]),
        Invocation::Operation(Operation::HelpSkills { json: false })
    );
    assert_eq!(
        parse(&["park", "help", "--skill", "--json"]),
        Invocation::Operation(Operation::HelpSkills { json: true })
    );
}

#[test]
fn parses_general_help() {
    assert_eq!(
        parse(&["park", "help"]),
        Invocation::Operation(Operation::Help)
    );
}

#[test]
fn json_help_requires_a_topic() {
    assert!(parse_invocation(["park", "help", "--json"] as [&str; 3]).is_err());
}

#[test]
fn distinguishes_a_long_operation_alias_from_a_dash_prefixed_name() {
    assert_eq!(
        parse(&["park", "--status", "--", "./server"]),
        Invocation::Launch {
            name: "--status".into(),
            env_files: vec![],
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
            env_files: vec![],
            command: vec!["cargo".into(), "run".into()],
        }
    );
}

#[test]
fn parses_wait_conditions_and_durations() {
    assert_eq!(
        parse(&[
            "park",
            "wait",
            "dev",
            "--state",
            "running",
            "--timeout",
            "250ms"
        ]),
        Invocation::Operation(Operation::Wait(WaitArgs {
            name: "dev".into(),
            state: Some(ProcessState::Running),
            match_text: None,
            exit: false,
            timeout: Some(250),
        }))
    );
    assert_eq!(
        parse(&["park", "wait", "dev", "--match", "ready", "--timeout", "2s"]),
        Invocation::Operation(Operation::Wait(WaitArgs {
            name: "dev".into(),
            state: None,
            match_text: Some("ready".to_owned()),
            exit: false,
            timeout: Some(2_000),
        }))
    );
}

#[test]
fn parses_environment_inputs_and_lifecycle_variants() {
    assert_eq!(
        parse(&["park", "dev", "--env-file", ".env", "--", "./server"]),
        Invocation::Launch {
            name: "dev".into(),
            env_files: vec![".env".into()],
            command: vec!["./server".into()],
        }
    );
    assert_eq!(
        parse(&[
            "park",
            "restart",
            "dev",
            "--recapture-env",
            "--env-file",
            ".env.local",
        ]),
        Invocation::Operation(Operation::Restart(RestartArgs {
            name: "dev".into(),
            recapture_env: true,
            env_files: vec![".env.local".into()],
        }))
    );
    assert_eq!(
        parse(&[
            "park",
            "start",
            "dev",
            "--env-file",
            ".env",
            "--",
            "./server",
        ]),
        Invocation::Operation(Operation::Start(StartArgs {
            name: "dev".into(),
            env_files: vec![".env".into()],
            command: vec!["./server".into()],
        }))
    );
    assert_eq!(
        parse(&[
            "park", "env", "dev", "--set", "A=B", "--unset", "C", "--json"
        ]),
        Invocation::Operation(Operation::Env(EnvArgs {
            name: "dev".into(),
            set: vec!["A=B".into()],
            unset: vec!["C".into()],
            json: true,
        }))
    );
}

#[test]
fn requires_one_wait_condition() {
    assert!(parse_invocation(["park", "wait", "dev"] as [&str; 3]).is_err());
    assert!(
        parse_invocation(["park", "wait", "dev", "--exit", "--state", "running"] as [&str; 6])
            .is_err()
    );
    assert!(parse_invocation(["park", "wait", "dev", "--state", "unknown"] as [&str; 5]).is_err());
}
