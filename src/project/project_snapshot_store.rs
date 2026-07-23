use std::{fs, path::PathBuf};

use crate::domain::ProjectId;

use super::{
    CatalogStore, ProjectArchive, ProjectCatalog,
    catalog_backup::{BackupError, ProjectSnapshot},
    catalog_backup_files::{io_error, write_immutable},
};

const PROJECT_SNAPSHOTS_DIRECTORY: &str = "project-snapshots";

impl CatalogStore {
    pub(crate) fn create_project_snapshot(
        &self,
        catalog: &mut ProjectCatalog,
        project: &ProjectId,
    ) -> Result<ProjectSnapshot, BackupError> {
        let archive = catalog.export_archive(project)?;
        let bytes = serde_yaml_ng::to_string(&archive)
            .expect("project archives serialize")
            .into_bytes();
        let path = self.project_snapshot_path(project, archive.project.revision);
        if path.exists() {
            let existing = read_yaml(&path)?;
            if existing != archive {
                return Err(io_error(
                    path,
                    std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        "snapshot revision contains different state",
                    ),
                ));
            }
        } else {
            write_immutable(&path, &bytes)?;
        }
        Ok(project_snapshot(&archive, bytes.len() as u64))
    }

    pub(crate) fn list_project_snapshots(
        &self,
        project: &ProjectId,
    ) -> Result<Vec<ProjectSnapshot>, BackupError> {
        let root = self
            .root
            .join(PROJECT_SNAPSHOTS_DIRECTORY)
            .join(project.as_str());
        let entries = match fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(source) => return Err(io_error(root, source)),
        };
        let mut snapshots = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| io_error(root.clone(), source))?;
            if entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "yaml")
            {
                let archive = read_yaml(&entry.path())?;
                archive.validated_import()?;
                let bytes = entry
                    .metadata()
                    .map_err(|source| io_error(entry.path(), source))?
                    .len();
                snapshots.push(project_snapshot(&archive, bytes));
            }
        }
        snapshots.sort_by_key(|snapshot| snapshot.revision);
        Ok(snapshots)
    }

    pub(crate) fn get_project_snapshot(
        &self,
        project: &ProjectId,
        revision: u64,
    ) -> Result<ProjectArchive, BackupError> {
        let path = self.project_snapshot_path(project, revision);
        if !path.is_file() {
            return Err(BackupError::SnapshotNotFound {
                project: project.clone(),
                revision,
            });
        }
        let archive = read_yaml(&path)?;
        archive.validated_import()?;
        Ok(archive)
    }

    fn project_snapshot_path(&self, project: &ProjectId, revision: u64) -> PathBuf {
        self.root
            .join(PROJECT_SNAPSHOTS_DIRECTORY)
            .join(project.as_str())
            .join(format!("{revision}.yaml"))
    }
}

fn project_snapshot(archive: &ProjectArchive, size_bytes: u64) -> ProjectSnapshot {
    ProjectSnapshot {
        project: archive.project.id.clone(),
        revision: archive.project.revision,
        size_bytes,
        summary: archive.summary(),
    }
}

fn read_yaml(path: &std::path::Path) -> Result<ProjectArchive, BackupError> {
    let bytes = std::fs::read(path).map_err(|source| io_error(path.to_owned(), source))?;
    serde_yaml_ng::from_slice(&bytes).map_err(|source| {
        io_error(
            path.to_owned(),
            std::io::Error::new(std::io::ErrorKind::InvalidData, source),
        )
    })
}

#[cfg(test)]
mod tests {
    use crate::domain::ProjectId;

    use super::{CatalogStore, ProjectCatalog};

    #[test]
    fn same_revision_is_idempotent_but_never_overwrites_different_content() {
        let root = std::env::temp_dir().join(format!(
            "optimist-project-snapshot-{}",
            uuid::Uuid::new_v4()
        ));
        let store = CatalogStore::new(root.clone());
        let project_id = ProjectId::new("A").unwrap();

        let mut first_catalog = ProjectCatalog::new();
        first_catalog.create("Delivery".to_owned()).unwrap();
        let first = store
            .create_project_snapshot(&mut first_catalog, &project_id)
            .unwrap();
        let repeated = store
            .create_project_snapshot(&mut first_catalog, &project_id)
            .unwrap();
        assert_eq!(repeated, first);

        let mut divergent_catalog = ProjectCatalog::new();
        divergent_catalog.create("Security".to_owned()).unwrap();
        assert!(
            store
                .create_project_snapshot(&mut divergent_catalog, &project_id)
                .is_err()
        );
        assert_eq!(
            store
                .get_project_snapshot(&project_id, 0)
                .unwrap()
                .project
                .name,
            "Delivery"
        );

        std::fs::remove_dir_all(root).unwrap();
    }
}
