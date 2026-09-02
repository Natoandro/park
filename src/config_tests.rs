use super::*;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

fn environment(root: &std::path::Path) -> XdgEnvironment {
    XdgEnvironment {
        config_home: Some(root.join("config")),
        state_home: Some(root.join("state")),
        runtime_dir: Some(root.join("runtime")),
        home: Some(root.join("home")),
    }
}

fn temporary_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "park-config-{}-{}",
        std::process::id(),
        NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).expect("test root should be created");
    root
}

#[test]
fn defaults_when_configuration_file_is_missing() {
    let root = temporary_root();
    let config = Config::load(&environment(&root)).expect("missing config should be allowed");
    assert_eq!(config, Config::default());
    assert_eq!(
        config_path(&environment(&root)),
        Some(root.join("config/park/config.toml"))
    );
    fs::remove_dir_all(root).expect("test root should be removed");
}

#[test]
fn loads_partial_configuration_over_defaults() {
    let root = temporary_root();
    let path = config_path(&environment(&root)).expect("config path should exist");
    fs::create_dir_all(path.parent().expect("config parent should exist"))
        .expect("config parent should be created");
    fs::write(&path, "[daemon.reexec]\nactive_processes = \"restart\"\n")
        .expect("config should be written");

    let config = Config::load(&environment(&root)).expect("config should load");
    assert_eq!(
        config.daemon.reexec.active_processes,
        ActiveProcessPolicy::Restart
    );
    assert_eq!(config.managed_processes.restart, RestartConfig::default());
    fs::remove_dir_all(root).expect("test root should be removed");
}

#[test]
fn defaults_reexec_active_processes_to_defer() {
    let config = Config::default();
    assert_eq!(
        config.daemon.reexec.effective_active_processes(false),
        ActiveProcessPolicy::Defer
    );
    assert_eq!(
        config.daemon.reexec.effective_active_processes(true),
        ActiveProcessPolicy::Restart
    );
}

#[test]
fn restart_policy_is_opt_in_and_force_overrides_defer() {
    let config = Config {
        daemon: DaemonConfig {
            reexec: ReexecConfig {
                active_processes: ActiveProcessPolicy::Restart,
            },
        },
        ..Config::default()
    };
    assert_eq!(
        config.daemon.reexec.effective_active_processes(false),
        ActiveProcessPolicy::Restart
    );
    assert_eq!(
        config.daemon.reexec.effective_active_processes(true),
        ActiveProcessPolicy::Restart
    );
}

#[test]
fn parses_restart_policy_and_bounded_backoff() {
    let config: Config = toml::from_str(
        "[managed_processes.restart]\npolicy = \"on-failure\"\nmax_attempts = 5\ninitial_delay = \"500ms\"\nmax_delay = \"2s\"\nmultiplier = 1.5\n",
    )
    .expect("restart configuration should parse");
    let backoff = config
        .managed_processes
        .restart
        .backoff()
        .expect("restart backoff should be valid");

    assert_eq!(
        config.managed_processes.restart.policy,
        RestartPolicy::OnFailure
    );
    assert_eq!(config.managed_processes.restart.max_attempts, 5);
    assert_eq!(backoff.initial_delay, Duration::from_millis(500));
    assert_eq!(backoff.max_delay, Duration::from_secs(2));
    assert_eq!(backoff.multiplier, 1.5);
}

#[test]
fn rejects_invalid_restart_backoff_configuration() {
    let invalid_configs = [
        "[managed_processes.restart]\ninitial_delay = \"1h\"\n",
        "[managed_processes.restart]\ninitial_delay = \"2s\"\nmax_delay = \"1s\"\n",
        "[managed_processes.restart]\nmultiplier = 0.5\n",
    ];
    for contents in invalid_configs {
        let config: Config = toml::from_str(contents).expect("TOML should parse");
        assert!(
            config.validate().is_err(),
            "configuration should be rejected"
        );
    }
}

#[test]
fn serializes_active_process_policy_using_documented_values() {
    let defer = toml::to_string(&ReexecConfig::default()).expect("config should serialize");
    assert_eq!(defer, "active_processes = \"defer\"\n");

    let restart = toml::to_string(&ReexecConfig {
        active_processes: ActiveProcessPolicy::Restart,
    })
    .expect("config should serialize");
    assert_eq!(restart, "active_processes = \"restart\"\n");
}

#[test]
fn reports_invalid_configuration() {
    let root = temporary_root();
    let path = config_path(&environment(&root)).expect("config path should exist");
    fs::create_dir_all(path.parent().expect("config parent should exist"))
        .expect("config parent should be created");
    fs::write(&path, "[daemon.reexec]\nactive_processes = \"invalid\"\n")
        .expect("config should be written");

    assert!(matches!(
        Config::load(&environment(&root)),
        Err(ConfigError::Parse { .. })
    ));
    fs::remove_dir_all(root).expect("test root should be removed");
}

#[test]
fn uses_home_config_fallback() {
    let environment = XdgEnvironment {
        config_home: None,
        state_home: None,
        runtime_dir: None,
        home: Some(PathBuf::from("/home/user")),
    };
    assert_eq!(
        config_path(&environment),
        Some(PathBuf::from("/home/user/.config/park/config.toml"))
    );
}

#[test]
fn prefers_xdg_config_home_over_home() {
    let environment = XdgEnvironment {
        config_home: Some(PathBuf::from("/config")),
        state_home: None,
        runtime_dir: None,
        home: Some(PathBuf::from("/home/user")),
    };
    assert_eq!(
        config_path(&environment),
        Some(PathBuf::from("/config/park/config.toml"))
    );
}

#[test]
fn reports_unreadable_configuration_path() {
    let root = temporary_root();
    let path = config_path(&environment(&root)).expect("config path should exist");
    fs::create_dir_all(&path).expect("config path directory should be created");

    assert!(matches!(
        Config::load(&environment(&root)),
        Err(ConfigError::Read { .. })
    ));
    fs::remove_dir_all(root).expect("test root should be removed");
}
