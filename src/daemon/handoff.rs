use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::descriptors::{DescriptorTable, MAX_DESCRIPTOR_TABLE_BYTES};

// Version zero is the first versioned handoff format. It is distinct from the
// IPC protocol version and remains stable until the format changes.
pub const HANDOFF_VERSION: u16 = 0;
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
pub const MANIFEST_FILE_NAME: &str = "daemon.handoff.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandoffManifest {
    pub version: u16,
    pub generation: u64,
    pub expires_at: u64,
    pub descriptors: DescriptorTable,
}

impl HandoffManifest {
    pub fn new(generation: u64, expires_at: u64, descriptors: DescriptorTable) -> Self {
        Self {
            version: HANDOFF_VERSION,
            generation,
            expires_at,
            descriptors,
        }
    }

    pub fn validate(&self, now: u64) -> Result<(), HandoffError> {
        if self.version != HANDOFF_VERSION {
            return Err(HandoffError::UnsupportedVersion(self.version));
        }
        if self.generation == 0 {
            return Err(HandoffError::Invalid("generation must be non-zero"));
        }
        if self.expires_at <= now {
            return Err(HandoffError::Expired {
                expires_at: self.expires_at,
                now,
            });
        }
        self.descriptors.validate()?;
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>, HandoffError> {
        let payload = serde_json::to_vec(self).map_err(HandoffError::Serialize)?;
        if payload.len() > MAX_MANIFEST_BYTES {
            return Err(HandoffError::TooLarge(payload.len()));
        }
        Ok(payload)
    }

    pub fn write_atomic(&self, path: &Path) -> Result<(), HandoffError> {
        self.validate(0)?;
        let payload = self.encode()?;
        let temporary = temporary_path(path);
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|source| HandoffError::Io {
                    operation: "create handoff manifest",
                    path: temporary.clone(),
                    source,
                })?;
            set_private_permissions(&temporary)?;
            file.write_all(&payload)
                .map_err(|source| HandoffError::Io {
                    operation: "write handoff manifest",
                    path: temporary.clone(),
                    source,
                })?;
            file.sync_all().map_err(|source| HandoffError::Io {
                operation: "sync handoff manifest",
                path: temporary.clone(),
                source,
            })?;
            fs::rename(&temporary, path).map_err(|source| HandoffError::Io {
                operation: "replace handoff manifest",
                path: path.to_path_buf(),
                source,
            })?;
            set_private_permissions(path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    pub fn read(path: &Path, now: u64) -> Result<Self, HandoffError> {
        let metadata = fs::metadata(path).map_err(|source| HandoffError::Io {
            operation: "inspect handoff manifest",
            path: path.to_path_buf(),
            source,
        })?;
        ensure_private_permissions(path, &metadata)?;
        if metadata.len() > MAX_MANIFEST_BYTES as u64 {
            return Err(HandoffError::TooLarge(metadata.len() as usize));
        }
        let mut file = fs::File::open(path).map_err(|source| HandoffError::Io {
            operation: "open handoff manifest",
            path: path.to_path_buf(),
            source,
        })?;
        let mut payload = Vec::with_capacity(metadata.len() as usize);
        file.read_to_end(&mut payload)
            .map_err(|source| HandoffError::Io {
                operation: "read handoff manifest",
                path: path.to_path_buf(),
                source,
            })?;
        if payload.len() > MAX_MANIFEST_BYTES {
            return Err(HandoffError::TooLarge(payload.len()));
        }
        let manifest: Self = serde_json::from_slice(&payload).map_err(HandoffError::Deserialize)?;
        manifest.validate(now)?;
        Ok(manifest)
    }

    pub fn remove(path: &Path) -> Result<(), HandoffError> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(HandoffError::Io {
                operation: "remove handoff manifest",
                path: path.to_path_buf(),
                source,
            }),
        }
    }
}

pub fn manifest_path(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join(MANIFEST_FILE_NAME)
}

fn temporary_path(path: &Path) -> PathBuf {
    path.with_extension(format!("json.{}.tmp", std::process::id()))
}

fn set_private_permissions(path: &Path) -> Result<(), HandoffError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|source| {
            HandoffError::Io {
                operation: "set handoff manifest permissions",
                path: path.to_path_buf(),
                source,
            }
        })?;
    }
    Ok(())
}

fn ensure_private_permissions(path: &Path, metadata: &fs::Metadata) -> Result<(), HandoffError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o777 != 0o600 {
            return Err(HandoffError::Io {
                operation: "validate private handoff manifest permissions",
                path: path.to_path_buf(),
                source: io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "manifest is not owner-only",
                ),
            });
        }
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum HandoffError {
    #[error("unsupported handoff version {0}")]
    UnsupportedVersion(u16),
    #[error("handoff manifest is expired at {expires_at}, current time is {now}")]
    Expired { expires_at: u64, now: u64 },
    #[error("invalid handoff manifest: {0}")]
    Invalid(&'static str),
    #[error("handoff manifest is too large: {0} bytes")]
    TooLarge(usize),
    #[error("could not serialize handoff manifest: {0}")]
    Serialize(#[source] serde_json::Error),
    #[error("could not decode handoff manifest: {0}")]
    Deserialize(#[source] serde_json::Error),
    #[error("could not {operation} {path:?}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    #[error(transparent)]
    Descriptors(#[from] super::descriptors::DescriptorError),
}

const _: usize = MAX_DESCRIPTOR_TABLE_BYTES;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::descriptors::{DescriptorEntry, DescriptorRole};
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn manifest() -> HandoffManifest {
        HandoffManifest::new(
            4,
            2_000,
            DescriptorTable {
                entries: vec![
                    DescriptorEntry {
                        fd: 3,
                        role: DescriptorRole::Listener,
                    },
                    DescriptorEntry {
                        fd: 4,
                        role: DescriptorRole::DaemonLock,
                    },
                ],
            },
        )
    }

    #[test]
    fn round_trips_an_atomic_private_manifest() {
        let root = std::env::temp_dir().join(format!("park-handoff-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("test directory should be created");
        let path = manifest_path(&root);
        manifest()
            .write_atomic(&path)
            .expect("manifest should write");
        let loaded = HandoffManifest::read(&path, 1_000).expect("manifest should read");
        assert_eq!(loaded, manifest());
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&path)
                .expect("manifest metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        HandoffManifest::remove(&path).expect("manifest should remove");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_expired_manifests() {
        let mut value = manifest();
        value.expires_at = 10;
        assert!(matches!(
            value.validate(10),
            Err(HandoffError::Expired { .. })
        ));
    }

    #[test]
    fn rejects_oversized_and_non_private_manifest_files() {
        let root =
            std::env::temp_dir().join(format!("park-handoff-invalid-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("test directory should be created");
        let path = manifest_path(&root);
        fs::write(&path, vec![b'x'; MAX_MANIFEST_BYTES + 1]).expect("oversized file should write");
        assert!(matches!(
            HandoffManifest::read(&path, 1),
            Err(HandoffError::Io { .. }) | Err(HandoffError::TooLarge(_))
        ));
        let _ = fs::remove_file(&path);
        manifest()
            .write_atomic(&path)
            .expect("manifest should write");
        #[cfg(unix)]
        {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
                .expect("permissions should change");
            assert!(matches!(
                HandoffManifest::read(&path, 1),
                Err(HandoffError::Io { .. })
            ));
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn manifest_path_is_under_runtime_directory() {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock");
        assert!(manifest_path(Path::new("/run/user/1000/park")).ends_with(MANIFEST_FILE_NAME));
        assert!(now.as_secs() > 0);
    }
}
