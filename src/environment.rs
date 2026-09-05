use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentEntry {
    #[serde(with = "crate::os_string")]
    pub key: OsString,
    #[serde(with = "crate::os_string")]
    pub value: OsString,
}

impl EnvironmentEntry {
    pub fn new(key: OsString, value: OsString) -> Result<Self, EnvironmentError> {
        validate_key(&key)?;
        Ok(Self { key, value })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentOverride {
    #[serde(with = "crate::os_string")]
    pub key: OsString,
    #[serde(with = "crate::os_string::option")]
    pub value: Option<OsString>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentCapture {
    pub entries: Vec<EnvironmentEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentSpec {
    pub capture: EnvironmentCapture,
    #[serde(with = "crate::os_string::vec")]
    pub dotenv_files: Vec<OsString>,
    pub overrides: Vec<EnvironmentOverride>,
}

impl Default for EnvironmentSpec {
    fn default() -> Self {
        Self::from_capture(EnvironmentCapture::default(), Vec::new())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEnvironment {
    entries: Vec<EnvironmentEntry>,
}

impl EnvironmentCapture {
    pub fn from_process() -> Result<Self, EnvironmentError> {
        let mut entries = std::env::vars_os()
            .map(|(key, value)| EnvironmentEntry::new(key, value))
            .collect::<Result<Vec<_>, _>>()?;
        sort_entries(&mut entries);
        Ok(Self { entries })
    }
}

impl EnvironmentSpec {
    pub fn from_capture(capture: EnvironmentCapture, dotenv_files: Vec<OsString>) -> Self {
        Self {
            capture,
            dotenv_files,
            overrides: Vec::new(),
        }
    }

    pub fn resolve(&self, project_path: &Path) -> Result<ResolvedEnvironment, EnvironmentError> {
        self.validate()?;
        let mut values = HashMap::<OsString, OsString>::new();
        for path in &self.dotenv_files {
            let path = resolve_dotenv_path(project_path, path);
            let contents = fs::read_to_string(&path).map_err(|source| EnvironmentError::Io {
                path: path.clone(),
                source,
            })?;
            for entry in parse_dotenv(&contents).map_err(|source| EnvironmentError::Dotenv {
                path: path.clone(),
                source,
            })? {
                values.insert(entry.key, entry.value);
            }
        }
        for entry in &self.capture.entries {
            values.insert(entry.key.clone(), entry.value.clone());
        }
        for item in &self.overrides {
            validate_key(&item.key)?;
            match &item.value {
                Some(value) => {
                    values.insert(item.key.clone(), value.clone());
                }
                None => {
                    values.remove(&item.key);
                }
            }
        }
        let mut entries = values
            .into_iter()
            .map(|(key, value)| EnvironmentEntry { key, value })
            .collect::<Vec<_>>();
        sort_entries(&mut entries);
        Ok(ResolvedEnvironment { entries })
    }

    pub(crate) fn validate(&self) -> Result<(), EnvironmentError> {
        let mut capture_keys = std::collections::HashSet::new();
        for entry in &self.capture.entries {
            validate_key(&entry.key)?;
            if !capture_keys.insert(entry.key.clone()) {
                return Err(EnvironmentError::DuplicateKey {
                    key: entry.key.to_string_lossy().into_owned(),
                });
            }
        }
        let mut override_keys = std::collections::HashSet::new();
        for item in &self.overrides {
            validate_key(&item.key)?;
            if !override_keys.insert(item.key.clone()) {
                return Err(EnvironmentError::DuplicateKey {
                    key: item.key.to_string_lossy().into_owned(),
                });
            }
        }
        if self.dotenv_files.iter().any(|path| path.is_empty()) {
            return Err(EnvironmentError::EmptyPath);
        }
        Ok(())
    }

    pub fn set(&mut self, key: OsString, value: OsString) -> Result<(), EnvironmentError> {
        validate_key(&key)?;
        self.overrides.retain(|item| item.key != key);
        self.overrides.push(EnvironmentOverride {
            key,
            value: Some(value),
        });
        sort_overrides(&mut self.overrides);
        Ok(())
    }

    pub fn unset(&mut self, key: OsString) -> Result<(), EnvironmentError> {
        validate_key(&key)?;
        self.overrides.retain(|item| item.key != key);
        self.overrides
            .push(EnvironmentOverride { key, value: None });
        sort_overrides(&mut self.overrides);
        Ok(())
    }
}

impl ResolvedEnvironment {
    pub fn entries(&self) -> &[EnvironmentEntry] {
        &self.entries
    }

    pub fn apply_to_command(&self, command: &mut tokio::process::Command) {
        command.env_clear();
        for entry in &self.entries {
            command.env(&entry.key, &entry.value);
        }
    }

    pub fn display_value(&self) -> serde_json::Value {
        serde_json::json!({
            "variables": self.entries.iter().map(|entry| serde_json::json!({
                "key": crate::os_string::encode_for_display(&entry.key),
                "value": crate::os_string::encode_for_display(&entry.value),
            })).collect::<Vec<_>>()
        })
    }
}

fn resolve_dotenv_path(project_path: &Path, path: &OsStr) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        project_path.join(path)
    }
}

fn parse_dotenv(contents: &str) -> Result<Vec<EnvironmentEntry>, DotenvError> {
    let mut entries = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        let number = index + 1;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let (key, value) = line
            .split_once('=')
            .ok_or(DotenvError::Syntax { line: number })?;
        let key = OsString::from(key.trim());
        validate_key(&key).map_err(|_| DotenvError::Key { line: number })?;
        let value = parse_value(value.trim(), number)?;
        entries.push(EnvironmentEntry { key, value });
    }
    Ok(entries)
}

fn parse_value(value: &str, line: usize) -> Result<OsString, DotenvError> {
    if value.contains("$(") || value.contains('`') {
        return Err(DotenvError::ShellSyntax { line });
    }
    if value.starts_with('"') || value.starts_with('\'') {
        let quote = value.as_bytes()[0] as char;
        let Some(relative_end) = value[1..].rfind(quote) else {
            return Err(DotenvError::Quote { line });
        };
        let end = relative_end + 1;
        if !value[end + 1..].trim().is_empty() && !value[end + 1..].trim_start().starts_with('#') {
            return Err(DotenvError::Quote { line });
        }
        let inner = &value[1..end];
        if inner.contains('\n') || inner.contains('\r') {
            return Err(DotenvError::Quote { line });
        }
        return Ok(OsString::from(inner));
    }
    let value = value
        .split_once(" #")
        .map_or(value, |(value, _)| value.trim_end());
    Ok(OsString::from(value))
}

fn validate_key(key: &OsStr) -> Result<(), EnvironmentError> {
    let bytes = key.as_bytes();
    if bytes.is_empty()
        || bytes.contains(&b'=')
        || bytes.contains(&0)
        || bytes.iter().any(u8::is_ascii_whitespace)
    {
        return Err(EnvironmentError::InvalidKey {
            key: key.to_string_lossy().into_owned(),
        });
    }
    Ok(())
}

fn sort_entries(entries: &mut [EnvironmentEntry]) {
    entries.sort_by(|left, right| left.key.as_bytes().cmp(right.key.as_bytes()));
}

fn sort_overrides(overrides: &mut [EnvironmentOverride]) {
    overrides.sort_by(|left, right| left.key.as_bytes().cmp(right.key.as_bytes()));
}

#[derive(Debug, Error)]
pub enum EnvironmentError {
    #[error("invalid environment variable name {key:?}")]
    InvalidKey { key: String },
    #[error("environment variable {key:?} is specified more than once")]
    DuplicateKey { key: String },
    #[error("environment file path must not be empty")]
    EmptyPath,
    #[error("could not read environment file {path:?}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("could not parse environment file {path:?}: {source}")]
    Dotenv { path: PathBuf, source: DotenvError },
}

#[derive(Debug, Error)]
pub enum DotenvError {
    #[error("invalid assignment on line {line}")]
    Syntax { line: usize },
    #[error("invalid variable name on line {line}")]
    Key { line: usize },
    #[error("unterminated or malformed quoted value on line {line}")]
    Quote { line: usize },
    #[error("shell syntax is not supported on line {line}")]
    ShellSyntax { line: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_dotenv_capture_and_explicit_precedence() {
        let path =
            std::env::temp_dir().join(format!("park-environment-{}-{}.env", std::process::id(), 1));
        fs::write(&path, "A=dotenv\nB=dotenv\nC=quoted value\n")
            .expect("dotenv file should be written");
        let capture = EnvironmentCapture {
            entries: vec![
                EnvironmentEntry::new("A".into(), "captured".into()).unwrap(),
                EnvironmentEntry::new("B".into(), "captured".into()).unwrap(),
            ],
        };
        let mut spec = EnvironmentSpec::from_capture(capture, vec![path.clone().into()]);
        spec.set("D".into(), "explicit".into()).unwrap();
        spec.unset("B".into()).unwrap();
        let resolved = spec
            .resolve(Path::new("/"))
            .expect("environment should resolve");
        assert_eq!(
            resolved
                .entries()
                .iter()
                .map(|entry| (entry.key.to_string_lossy(), entry.value.to_string_lossy()))
                .collect::<Vec<_>>(),
            vec![
                ("A".into(), "captured".into()),
                ("C".into(), "quoted value".into()),
                ("D".into(), "explicit".into()),
            ]
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_shell_like_dotenv_syntax() {
        assert!(parse_dotenv("A=$(touch /tmp/nope)\n").is_err());
        assert!(parse_dotenv("A=\"unterminated\n").is_err());
        assert!(parse_dotenv("not-an-assignment\n").is_err());
    }
}
