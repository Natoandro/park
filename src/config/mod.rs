use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

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
        let config: Self = toml::from_str(&contents).map_err(|source| ConfigError::Parse {
            path: path.clone(),
            source,
        })?;
        config
            .validate()
            .map_err(|message| ConfigError::Invalid { path, message })?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), String> {
        self.managed_processes.restart.validate()
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

impl ReexecConfig {
    /// Return the policy for one re-exec request, applying the command-line
    /// override without changing the configured default.
    pub const fn effective_active_processes(&self, force: bool) -> ActiveProcessPolicy {
        if force {
            ActiveProcessPolicy::Restart
        } else {
            self.active_processes
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

impl RestartConfig {
    pub fn validate(&self) -> Result<(), String> {
        let backoff = self.backoff()?;
        if backoff.initial_delay > backoff.max_delay {
            return Err(format!(
                "initial_delay ({}) must not exceed max_delay ({})",
                self.initial_delay, self.max_delay
            ));
        }
        Ok(())
    }

    pub fn backoff(&self) -> Result<RestartBackoff, String> {
        let initial_delay = parse_duration(&self.initial_delay).map_err(|message| {
            format!("invalid initial_delay {:?}: {message}", self.initial_delay)
        })?;
        let max_delay = parse_duration(&self.max_delay)
            .map_err(|message| format!("invalid max_delay {:?}: {message}", self.max_delay))?;
        if !self.multiplier.is_finite() || self.multiplier < 1.0 {
            return Err(
                "multiplier must be a finite number greater than or equal to 1.0".to_owned(),
            );
        }
        Ok(RestartBackoff {
            initial_delay,
            max_delay,
            multiplier: self.multiplier,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RestartBackoff {
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    let (number, multiplier) = if let Some(number) = value.strip_suffix("ms") {
        (number, 1_u64)
    } else if let Some(number) = value.strip_suffix('s') {
        (number, 1_000_u64)
    } else if let Some(number) = value.strip_suffix('m') {
        (number, 60_000_u64)
    } else {
        return Err("use a non-negative integer ending in ms, s, or m".to_owned());
    };
    let number = number
        .parse::<u64>()
        .map_err(|_| "use a non-negative integer ending in ms, s, or m".to_owned())?;
    let milliseconds = number
        .checked_mul(multiplier)
        .ok_or_else(|| "duration is too large".to_owned())?;
    Ok(Duration::from_millis(milliseconds))
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
    #[error("invalid configuration file {path:?}: {message}")]
    Invalid { path: PathBuf, message: String },
}

#[cfg(test)]
mod tests;
