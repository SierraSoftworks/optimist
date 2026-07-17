use crate::project::{CatalogBackup, CatalogRestore, ProjectArchive, ProjectSnapshot};

use super::{output::OutputFormat, output_json, output_table_backup};

impl OutputFormat {
    pub(super) fn catalog_backup(
        self,
        backup: &CatalogBackup,
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table_backup::catalog_backups(std::slice::from_ref(
                backup,
            ))),
            Self::Json | Self::Jsonl => output_json::serialize(backup),
        }
    }

    pub(super) fn catalog_backups(
        self,
        backups: &[CatalogBackup],
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table_backup::catalog_backups(backups)),
            Self::Json => output_json::serialize(backups),
            Self::Jsonl => output_json::lines(backups),
        }
    }

    pub(super) fn catalog_restore(
        self,
        restore: &CatalogRestore,
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table_backup::catalog_restore(restore)),
            Self::Json | Self::Jsonl => output_json::serialize(restore),
        }
    }

    pub(super) fn project_snapshot(
        self,
        snapshot: &ProjectSnapshot,
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table_backup::project_snapshots(
                std::slice::from_ref(snapshot),
            )),
            Self::Json | Self::Jsonl => output_json::serialize(snapshot),
        }
    }

    pub(super) fn project_snapshots(
        self,
        snapshots: &[ProjectSnapshot],
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table_backup::project_snapshots(snapshots)),
            Self::Json => output_json::serialize(snapshots),
            Self::Jsonl => output_json::lines(snapshots),
        }
    }

    pub(super) fn project_archive(
        self,
        archive: &ProjectArchive,
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table_backup::project_archive(archive)),
            Self::Json | Self::Jsonl => output_json::serialize(archive),
        }
    }
}
