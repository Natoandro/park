use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;

use crate::process::ProcessKey;

use super::StorageError;

pub(super) fn create_new_file(path: &Path) -> Result<File, StorageError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                StorageError::RecordExists {
                    path: path.to_path_buf(),
                }
            } else {
                StorageError::Io {
                    operation: "create log",
                    path: path.to_path_buf(),
                    source,
                }
            }
        })
}

pub(super) fn remove_if_present(path: &Path) -> Result<(), StorageError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(StorageError::Io {
            operation: "remove file",
            path: path.to_path_buf(),
            source,
        }),
    }
}

pub(super) fn key_digest(key: &ProcessKey) -> String {
    use std::os::unix::ffi::OsStrExt;

    let mut first = 0xcbf29ce484222325_u64;
    let mut second = 0x84222325cbf29ce4_u64;
    for bytes in [
        key.project_path().as_os_str().as_bytes(),
        key.name().as_bytes(),
    ] {
        for byte in bytes {
            first = (first ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
            second = (second ^ u64::from(*byte)).wrapping_mul(0x100000001b3 ^ 0x9e3779b97f4a7c15);
        }
        first = (first ^ 0xff).wrapping_mul(0x100000001b3);
        second = (second ^ 0xff).wrapping_mul(0x100000001b3 ^ 0x9e3779b97f4a7c15);
    }
    format!("{first:016x}{second:016x}")
}

pub(super) fn key_log_prefix(key: &ProcessKey) -> String {
    key_digest(key)
}

pub(super) fn set_private_permissions(path: &Path) -> Result<(), StorageError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|source| {
            StorageError::Io {
                operation: "set private directory permissions",
                path: path.to_path_buf(),
                source,
            }
        })?;
    }
    Ok(())
}

pub(super) fn set_private_file_permissions(path: &Path) -> Result<(), StorageError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|source| {
            StorageError::Io {
                operation: "set private database permissions",
                path: path.to_path_buf(),
                source,
            }
        })?;
    }
    Ok(())
}
