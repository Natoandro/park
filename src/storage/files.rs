use std::fs::{self, File, OpenOptions, ReadDir};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::process::{ProcessKey, ProcessRecord};

use super::StorageError;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(super) fn read_records(entries: ReadDir) -> Result<Vec<ProcessRecord>, StorageError> {
    let mut records = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| StorageError::Io {
            operation: "read record entry",
            path: PathBuf::new(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        records.push(read_record(&path)?);
    }
    Ok(records)
}

pub(super) fn read_record(path: &Path) -> Result<ProcessRecord, StorageError> {
    let bytes = fs::read(path).map_err(|source| StorageError::Io {
        operation: "read record",
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| StorageError::InvalidRecord {
        path: path.to_path_buf(),
        source,
    })
}

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

pub(super) fn atomic_create(path: &Path, payload: &[u8]) -> Result<(), StorageError> {
    let temporary = write_temporary(path, payload)?;
    match fs::hard_link(&temporary, path) {
        Ok(()) => {
            remove_if_present(&temporary)?;
            Ok(())
        }
        Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(&temporary);
            Err(StorageError::RecordExists {
                path: path.to_path_buf(),
            })
        }
        Err(source) => {
            let _ = fs::remove_file(&temporary);
            Err(StorageError::Io {
                operation: "atomically create record",
                path: path.to_path_buf(),
                source,
            })
        }
    }
}

pub(super) fn atomic_replace(path: &Path, payload: &[u8]) -> Result<(), StorageError> {
    let temporary = write_temporary(path, payload)?;
    fs::rename(&temporary, path).map_err(|source| {
        let _ = fs::remove_file(&temporary);
        StorageError::Io {
            operation: "atomically replace record",
            path: path.to_path_buf(),
            source,
        }
    })
}

fn write_temporary(path: &Path, payload: &[u8]) -> Result<PathBuf, StorageError> {
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let temporary =
        path.with_file_name(format!(".{file_name}.tmp-{}-{counter}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|source| StorageError::Io {
            operation: "create temporary record",
            path: temporary.clone(),
            source,
        })?;
    file.write_all(payload).map_err(|source| StorageError::Io {
        operation: "write temporary record",
        path: temporary.clone(),
        source,
    })?;
    file.sync_all().map_err(|source| StorageError::Io {
        operation: "sync temporary record",
        path: temporary.clone(),
        source,
    })?;
    Ok(temporary)
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

fn key_digest(key: &ProcessKey) -> String {
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

pub(super) fn key_record_name(key: &ProcessKey) -> String {
    format!("{}.json", key_digest(key))
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
