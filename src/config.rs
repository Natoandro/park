use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::storage::XdgEnvironment;

const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub daemon: DaemonConfig,
    pub managed_processes: ManagedProcessesConfig,
}

impl Config {
    pub fn load(environment: &XdgEnvironment) -> Result<Self, ConfigError> {
        let Some(path) = config_path(environment) else {
            return Ok(Self::default());
        };
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(source) => return Err(ConfigError::Read { path, source }),
        };
        toml::from_str(&contents).map_err(|source| ConfigError::Parse { path, source })
    }
}

pub fn config_path(environment: &XdgEnvironment) -> Option<PathBuf> {
    let base = environment
        .config_home
        .as_deref()
        .filter(|path| !path.as_os_str().is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            environment
                .home
                .as_deref()
                .filter(|path| !path.as_os_str().is_empty())
                .map(|home| home.join(".config"))
        })?;
    Some(base.join("park").join(CONFIG_FILE_NAME))
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DaemonConfig {
    pub reexec: ReexecConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReexecConfig {
    pub active_processes: ActiveProcessPolicy,
}

impl Default for ReexecConfig {
    fn default() -> Self {
        Self {
            active_processes: ActiveProcessPolicy::Defer,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActiveProcessPolicy {
    #[default]
    Defer,
    Restart,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ManagedProcessesConfig {
    pub restart: RestartConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RestartConfig {
    pub policy: RestartPolicy,
    pub max_attempts: u32,
    pub initial_delay: String,
    pub max_delay: String,
    pub multiplier: f64,
}

impl Default for RestartConfig {
    fn default() -> Self {
        Self {
            policy: RestartPolicy::Never,
            max_attempts: 3,
            initial_delay: "250ms".to_owned(),
            max_delay: "30s".to_owned(),
            multiplier: 2.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RestartPolicy {
    #[serde(rename = "never")]
    #[default]
    Never,
    #[serde(rename = "on-failure")]
    OnFailure,
    #[serde(rename = "always")]
    Always,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not read configuration file {path:?}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("could not parse configuration file {path:?}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
}

#[cfg(test)]
mod tests {
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
}
