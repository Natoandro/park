use std::io;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MIN_INHERITED_FD: i32 = 3;
pub const MAX_INHERITED_FD: i32 = 1023;
pub const MAX_DESCRIPTOR_TABLE_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DescriptorRole {
    Listener,
    DaemonLock,
    ManagedStdout { record_key: String },
    ManagedStderr { record_key: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DescriptorEntry {
    pub fd: i32,
    pub role: DescriptorRole,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DescriptorTable {
    pub entries: Vec<DescriptorEntry>,
}

impl DescriptorTable {
    pub fn validate(&self) -> Result<(), DescriptorError> {
        if serde_json::to_vec(self)
            .map_err(DescriptorError::Serialize)?
            .len()
            > MAX_DESCRIPTOR_TABLE_BYTES
        {
            return Err(DescriptorError::TooLarge);
        }
        let mut fds = std::collections::HashSet::new();
        let mut roles = std::collections::HashSet::new();
        for entry in &self.entries {
            if !(MIN_INHERITED_FD..=MAX_INHERITED_FD).contains(&entry.fd) {
                return Err(DescriptorError::InvalidFd(entry.fd));
            }
            if !fds.insert(entry.fd) {
                return Err(DescriptorError::DuplicateFd(entry.fd));
            }
            let role = entry.role.identity();
            if !roles.insert(role.clone()) {
                return Err(DescriptorError::DuplicateRole(role));
            }
        }
        Ok(())
    }

    pub fn set_inheritable(&self) -> Result<(), DescriptorError> {
        self.validate()?;
        for entry in &self.entries {
            set_cloexec(entry.fd, false)?;
        }
        Ok(())
    }

    pub fn set_cloexec(&self) -> Result<(), DescriptorError> {
        self.validate()?;
        for entry in &self.entries {
            set_cloexec(entry.fd, true)?;
        }
        Ok(())
    }

    pub fn ensure_inheritable(&self) -> Result<(), DescriptorError> {
        self.validate()?;
        for entry in &self.entries {
            check_cloexec(entry.fd, false)?;
        }
        Ok(())
    }

    pub fn ensure_cloexec(&self) -> Result<(), DescriptorError> {
        self.validate()?;
        for entry in &self.entries {
            check_cloexec(entry.fd, true)?;
        }
        Ok(())
    }
}

impl DescriptorRole {
    fn identity(&self) -> String {
        match self {
            Self::Listener => "listener".to_owned(),
            Self::DaemonLock => "daemon_lock".to_owned(),
            Self::ManagedStdout { record_key } => format!("managed_stdout:{record_key}"),
            Self::ManagedStderr { record_key } => format!("managed_stderr:{record_key}"),
        }
    }
}

fn set_cloexec(fd: i32, enabled: bool) -> Result<(), DescriptorError> {
    #[cfg(unix)]
    {
        let flags = unsafe { nix::libc::fcntl(fd, nix::libc::F_GETFD) };
        if flags < 0 {
            return Err(DescriptorError::Io {
                operation: "read descriptor flags",
                fd,
                source: io::Error::last_os_error(),
            });
        }
        let updated = if enabled {
            flags | nix::libc::FD_CLOEXEC
        } else {
            flags & !nix::libc::FD_CLOEXEC
        };
        if unsafe { nix::libc::fcntl(fd, nix::libc::F_SETFD, updated) } < 0 {
            return Err(DescriptorError::Io {
                operation: "write descriptor flags",
                fd,
                source: io::Error::last_os_error(),
            });
        }
    }
    #[cfg(not(unix))]
    let _ = (fd, enabled);
    Ok(())
}

fn check_cloexec(fd: i32, expected: bool) -> Result<(), DescriptorError> {
    #[cfg(unix)]
    {
        let flags = unsafe { nix::libc::fcntl(fd, nix::libc::F_GETFD) };
        if flags < 0 {
            return Err(DescriptorError::Io {
                operation: "read descriptor flags",
                fd,
                source: io::Error::last_os_error(),
            });
        }
        let actual = flags & nix::libc::FD_CLOEXEC != 0;
        if actual != expected {
            return Err(DescriptorError::CloexecMismatch { fd, expected });
        }
    }
    #[cfg(not(unix))]
    let _ = (fd, expected);
    Ok(())
}

#[derive(Debug, Error)]
pub enum DescriptorError {
    #[error("descriptor table is too large")]
    TooLarge,
    #[error("descriptor {0} is outside the approved inherited range")]
    InvalidFd(i32),
    #[error("descriptor {0} appears more than once")]
    DuplicateFd(i32),
    #[error("descriptor role {0} appears more than once")]
    DuplicateRole(String),
    #[error("could not {operation} for descriptor {fd}: {source}")]
    Io {
        operation: &'static str,
        fd: i32,
        source: io::Error,
    },
    #[error("could not serialize descriptor table: {0}")]
    Serialize(#[source] serde_json::Error),
    #[error("descriptor {fd} has unexpected FD_CLOEXEC state; expected {expected}")]
    CloexecMismatch { fd: i32, expected: bool },
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::fd::AsRawFd;

    #[test]
    fn rejects_duplicate_fds_and_roles() {
        let duplicate_fd = DescriptorTable {
            entries: vec![
                DescriptorEntry {
                    fd: 3,
                    role: DescriptorRole::Listener,
                },
                DescriptorEntry {
                    fd: 3,
                    role: DescriptorRole::DaemonLock,
                },
            ],
        };
        assert!(matches!(
            duplicate_fd.validate(),
            Err(DescriptorError::DuplicateFd(3))
        ));

        let duplicate_role = DescriptorTable {
            entries: vec![
                DescriptorEntry {
                    fd: 3,
                    role: DescriptorRole::Listener,
                },
                DescriptorEntry {
                    fd: 4,
                    role: DescriptorRole::Listener,
                },
            ],
        };
        assert!(matches!(
            duplicate_role.validate(),
            Err(DescriptorError::DuplicateRole(_))
        ));
    }

    #[test]
    fn rejects_descriptors_outside_the_inherited_range() {
        let table = DescriptorTable {
            entries: vec![DescriptorEntry {
                fd: 2,
                role: DescriptorRole::Listener,
            }],
        };
        assert!(matches!(
            table.validate(),
            Err(DescriptorError::InvalidFd(2))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn toggles_and_verifies_cloexec_only_for_listed_descriptors() {
        let file = std::fs::File::open("/dev/null").expect("descriptor should open");
        let table = DescriptorTable {
            entries: vec![DescriptorEntry {
                fd: file.as_raw_fd(),
                role: DescriptorRole::Listener,
            }],
        };
        table
            .set_inheritable()
            .expect("descriptor should be inheritable");
        table
            .ensure_inheritable()
            .expect("inheritable state should verify");
        table.set_cloexec().expect("descriptor should be cloexec");
        table.ensure_cloexec().expect("cloexec state should verify");
    }
}
