use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectPath(PathBuf);

impl ProjectPath {
    pub(crate) fn from_canonical(path: PathBuf) -> Self {
        Self(path)
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn into_path(self) -> PathBuf {
        self.0
    }
}

pub fn resolve_current_project() -> Result<ProjectPath, ProjectResolutionError> {
    resolve_project(
        env::current_dir().map_err(|source| ProjectResolutionError::CurrentDirectory { source })?,
    )
}

pub fn resolve_project(path: impl AsRef<Path>) -> Result<ProjectPath, ProjectResolutionError> {
    let requested = path.as_ref().to_path_buf();
    let canonical =
        fs::canonicalize(&requested).map_err(|source| ProjectResolutionError::Canonicalize {
            path: requested.clone(),
            source,
        })?;

    if !canonical.is_dir() {
        return Err(ProjectResolutionError::NotDirectory { path: canonical });
    }

    Ok(ProjectPath::from_canonical(canonical))
}

#[derive(Debug, Error)]
pub enum ProjectResolutionError {
    #[error("could not determine the current directory: {source}")]
    CurrentDirectory { source: io::Error },
    #[error("could not canonicalize project path {path:?}: {source}")]
    Canonicalize { path: PathBuf, source: io::Error },
    #[error("project path is not a directory: {path:?}")]
    NotDirectory { path: PathBuf },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

    fn temporary_directory() -> PathBuf {
        let id = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!("park-phase2-{}-{id}", std::process::id()));
        fs::create_dir(&path).expect("temporary directory should be created");
        path
    }

    #[test]
    fn canonicalizes_relative_project_paths() {
        let resolved = resolve_project(".").expect("current directory should resolve");
        let expected = fs::canonicalize(".").expect("current directory should canonicalize");
        assert_eq!(resolved.as_path(), expected);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_aliases_resolve_to_the_same_project() {
        use std::os::unix::fs::symlink;

        let real = temporary_directory();
        let alias = real.with_file_name(format!("{}-alias", real.display()));
        symlink(&real, &alias).expect("symlink should be created");

        assert_eq!(
            resolve_project(&real).expect("real path should resolve"),
            resolve_project(&alias).expect("symlink path should resolve")
        );

        fs::remove_file(alias).expect("symlink should be removed");
        fs::remove_dir(real).expect("temporary directory should be removed");
    }

    #[test]
    fn rejects_a_non_directory_project_path() {
        let root = temporary_directory();
        let file = root.join("not-a-directory");
        fs::write(&file, b"test").expect("file should be created");

        assert!(matches!(
            resolve_project(&file),
            Err(ProjectResolutionError::NotDirectory { .. })
        ));

        fs::remove_file(file).expect("file should be removed");
        fs::remove_dir(root).expect("temporary directory should be removed");
    }
}
