use std::fs;

use uuid::Uuid;

use super::{
    CatalogStore, ProjectCatalog,
    catalog_backup::{BackupError, CatalogBackup},
    catalog_backup_files::{io_error, now_unix_ms, read_json, sync_directory, write_synced},
};

const BACKUPS_DIRECTORY: &str = "backups";
const BACKUP_CATALOG: &str = "catalog.json";
const BACKUP_METADATA: &str = "metadata.json";

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
        let catalog_bytes = fs::read(self.snapshot_path())
            .map_err(|source| io_error(self.snapshot_path(), source))?;
        write_synced(&staging.join(BACKUP_CATALOG), &catalog_bytes, false)?;
        let metadata = CatalogBackup {
            id,
            created_unix_ms: now_unix_ms(),
            size_bytes: catalog_bytes.len() as u64,
            projects: catalog.list(),
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
        let catalog_path = directory.join(BACKUP_CATALOG);
        let catalog_bytes = fs::metadata(&catalog_path)
            .map_err(|source| io_error(catalog_path.clone(), source))?
            .len();
        let catalog = self.load_file(&catalog_path, false)?;
        if metadata.size_bytes != catalog_bytes || metadata.projects != catalog.list() {
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
