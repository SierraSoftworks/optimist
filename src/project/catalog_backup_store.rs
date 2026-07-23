use std::fs;

use uuid::Uuid;

use super::{
    CatalogStore, ProjectCatalog,
    catalog_backup::{BackupError, CatalogBackup},
    catalog_backup_files::{io_error, now_unix_ms, read_json, sync_directory, write_synced},
};

const BACKUPS_DIRECTORY: &str = "backups";
const BACKUP_METADATA: &str = "metadata.json";
const PROJECTS_DIRECTORY: &str = "projects";
const PROJECT_FILES: [&str; 2] = ["meta.json", "project.yaml"];

impl CatalogStore {
    pub(crate) fn create_backup(
        &self,
        catalog: &mut ProjectCatalog,
    ) -> Result<CatalogBackup, BackupError> {
        self.save(catalog)?;
        let id = Uuid::new_v4();
        let backups = self.root.join(BACKUPS_DIRECTORY);
        fs::create_dir_all(&backups).map_err(|source| io_error(backups.clone(), source))?;
        let staging = backups.join(format!(".{id}.tmp"));
        let target = backups.join(id.to_string());
        fs::create_dir(&staging).map_err(|source| io_error(staging.clone(), source))?;
        let projects_directory = staging.join(PROJECTS_DIRECTORY);
        fs::create_dir(&projects_directory)
            .map_err(|source| io_error(projects_directory.clone(), source))?;
        let mut size_bytes = 0_u64;
        let projects = self.list_project_metadata()?;
        let source_projects = self.root.join(PROJECTS_DIRECTORY);
        for entry in fs::read_dir(&source_projects)
            .map_err(|source| io_error(source_projects.clone(), source))?
        {
            let entry = entry.map_err(|source| io_error(source_projects.clone(), source))?;
            if !entry
                .file_type()
                .map_err(|source| io_error(entry.path(), source))?
                .is_dir()
            {
                continue;
            }
            let source_directory = entry.path();
            let destination = projects_directory.join(entry.file_name());
            fs::create_dir(&destination).map_err(|source| io_error(destination.clone(), source))?;
            for name in PROJECT_FILES {
                let source = source_directory.join(name);
                let bytes = match fs::read(&source) {
                    Ok(bytes) => bytes,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(io_error(source, error)),
                };
                size_bytes = size_bytes.saturating_add(bytes.len() as u64);
                write_synced(&destination.join(name), &bytes, false)?;
            }
            sync_directory(&destination)?;
        }
        sync_directory(&projects_directory)?;
        let metadata = CatalogBackup {
            id,
            created_unix_ms: now_unix_ms(),
            size_bytes,
            projects,
        };
        write_synced(
            &staging.join(BACKUP_METADATA),
            &serde_json::to_vec(&metadata).expect("backup metadata serializes"),
            false,
        )?;
        sync_directory(&staging)?;
        fs::rename(&staging, &target).map_err(|source| io_error(target, source))?;
        sync_directory(&backups)?;
        Ok(metadata)
    }

    pub(crate) fn list_backups(&self) -> Result<Vec<CatalogBackup>, BackupError> {
        let root = self.root.join(BACKUPS_DIRECTORY);
        let entries = match fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(source) => return Err(io_error(root, source)),
        };
        let mut backups = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| io_error(root.clone(), source))?;
            if !entry
                .file_type()
                .map_err(|source| io_error(entry.path(), source))?
                .is_dir()
                || entry.file_name().to_string_lossy().starts_with('.')
            {
                continue;
            }
            backups.push(read_json(&entry.path().join(BACKUP_METADATA))?);
        }
        backups.sort_by_key(|backup: &CatalogBackup| (backup.created_unix_ms, backup.id));
        Ok(backups)
    }

    pub(crate) fn load_backup(
        &self,
        id: Uuid,
    ) -> Result<(CatalogBackup, ProjectCatalog), BackupError> {
        let directory = self.root.join(BACKUPS_DIRECTORY).join(id.to_string());
        if !directory.is_dir() {
            return Err(BackupError::BackupNotFound(id));
        }
        let metadata: CatalogBackup = read_json(&directory.join(BACKUP_METADATA))?;
        if metadata.id != id {
            return Err(BackupError::BackupNotFound(id));
        }
        let backup_store = CatalogStore::new(directory.clone());
        let catalog = backup_store.load()?;
        let size_bytes = backup_project_bytes(&directory.join(PROJECTS_DIRECTORY))?;
        if metadata.size_bytes != size_bytes || metadata.projects != catalog.list() {
            return Err(BackupError::Json {
                path: directory.join(BACKUP_METADATA),
                source: serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "backup metadata does not match the catalog snapshot",
                )),
            });
        }
        Ok((metadata, catalog))
    }
}

fn backup_project_bytes(directory: &std::path::Path) -> Result<u64, BackupError> {
    let mut total = 0_u64;
    for entry in
        fs::read_dir(directory).map_err(|source| io_error(directory.to_path_buf(), source))?
    {
        let entry = entry.map_err(|source| io_error(directory.to_path_buf(), source))?;
        if !entry
            .file_type()
            .map_err(|source| io_error(entry.path(), source))?
            .is_dir()
        {
            continue;
        }
        for name in PROJECT_FILES {
            match fs::metadata(entry.path().join(name)) {
                Ok(metadata) => total = total.saturating_add(metadata.len()),
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
                Err(source) => return Err(io_error(entry.path().join(name), source)),
            }
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_backups_preserve_deleted_project_allocator_tombstones() {
        let root = std::env::temp_dir().join(format!(
            "optimist-directory-backup-{}",
            uuid::Uuid::new_v4()
        ));
        let store = CatalogStore::new(root.clone());
        let mut catalog = ProjectCatalog::new();
        let first = catalog.create("Delivery".to_owned()).unwrap();
        let deleted = catalog.create("Temporary".to_owned()).unwrap();
        catalog.delete(&deleted.id).unwrap();
        store.save(&mut catalog).unwrap();

        let backup = store.create_backup(&mut catalog).unwrap();
        let (_, mut restored) = store.load_backup(backup.id).unwrap();
        assert_eq!(restored.list(), vec![first]);
        assert_eq!(restored.create("Next".to_owned()).unwrap().id.as_str(), "C");

        fs::remove_dir_all(root).unwrap();
    }
}
