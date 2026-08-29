use std::collections::HashMap;

use thiserror::Error;

use crate::process::{ProcessKey, ProcessRecord};

#[derive(Debug, Default)]
pub struct ProcessRegistry {
    records: HashMap<ProcessKey, ProcessRecord>,
}

impl ProcessRegistry {
    pub fn insert(&mut self, record: ProcessRecord) -> Result<(), RegistryError> {
        let key = record.key().clone();
        if self.records.contains_key(&key) {
            return Err(RegistryError::Duplicate { key });
        }
        self.records.insert(key, record);
        Ok(())
    }

    pub fn get(&self, key: &ProcessKey) -> Option<&ProcessRecord> {
        self.records.get(key)
    }

    pub fn remove(&mut self, key: &ProcessKey) -> Option<ProcessRecord> {
        self.records.remove(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ProcessRecord> {
        self.records.values()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("a process named {key_name:?} already exists in project {project_path:?}", key_name = .key.name(), project_path = .key.project_path())]
    Duplicate { key: ProcessKey },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LogPaths, ProjectPath};
    use std::ffi::OsString;
    use std::path::PathBuf;

    fn record(project: &str, name: &str) -> ProcessRecord {
        let project = ProjectPath::from_canonical(PathBuf::from(project));
        let working_directory = project.as_path().to_path_buf();
        let key = ProcessKey::new(project, OsString::from(name));
        ProcessRecord::new(
            key,
            working_directory,
            OsString::from("server"),
            vec![],
            1,
            LogPaths {
                stdout: PathBuf::from("stdout.log"),
                stderr: PathBuf::from("stderr.log"),
            },
        )
    }

    #[test]
    fn rejects_duplicate_name_in_the_same_project() {
        let mut registry = ProcessRegistry::default();
        registry
            .insert(record("/project", "dev"))
            .expect("first record should insert");

        assert!(matches!(
            registry.insert(record("/project", "dev")),
            Err(RegistryError::Duplicate { .. })
        ));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn allows_identical_names_in_distinct_projects() {
        let mut registry = ProcessRegistry::default();
        registry
            .insert(record("/shop", "dev"))
            .expect("shop record should insert");
        registry
            .insert(record("/api", "dev"))
            .expect("api record should insert");

        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn lookup_requires_the_complete_process_key() {
        let mut registry = ProcessRegistry::default();
        let record = record("/project", "dev");
        let key = record.key().clone();
        registry.insert(record).expect("record should insert");

        assert_eq!(
            registry.get(&key).expect("key should find record").key(),
            &key
        );
    }
}
