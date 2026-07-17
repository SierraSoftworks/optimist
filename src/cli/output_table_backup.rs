use crate::project::{CatalogBackup, CatalogRestore, ProjectArchive, ProjectSnapshot};

pub(super) fn catalog_backups(backups: &[CatalogBackup]) -> String {
    rows(
        "ID\tCREATED_UNIX_MS\tSIZE_BYTES\tPROJECTS",
        backups.iter().map(|backup| {
            format!(
                "{}\t{}\t{}\t{}",
                backup.id,
                backup.created_unix_ms,
                backup.size_bytes,
                backup.projects.len()
            )
        }),
    )
}

pub(super) fn catalog_restore(restore: &CatalogRestore) -> String {
    format!(
        "RESTORED_BACKUP\tSAFETY_BACKUP\tPROJECTS\n{}\t{}\t{}",
        restore.restored.id,
        restore.safety_backup.id,
        restore.projects.len()
    )
}

pub(super) fn project_snapshots(snapshots: &[ProjectSnapshot]) -> String {
    rows(
        "PROJECT\tREVISION\tSIZE_BYTES\tENTITIES\tEDGES\tSCENARIOS",
        snapshots.iter().map(|snapshot| {
            format!(
                "{}\t{}\t{}\t{}\t{}\t{}",
                snapshot.project,
                snapshot.revision,
                snapshot.size_bytes,
                snapshot.summary.entities,
                snapshot.summary.edges,
                snapshot.summary.scenarios
            )
        }),
    )
}

pub(super) fn project_archive(archive: &ProjectArchive) -> String {
    format!(
        "PROJECT\tREVISION\tENTITIES\tEDGES\tSCENARIOS\n{}\t{}\t{}\t{}\t{}",
        archive.project.id,
        archive.project.revision,
        archive.summary.entities,
        archive.summary.edges,
        archive.summary.scenarios
    )
}

fn rows(header: &str, rows: impl Iterator<Item = String>) -> String {
    std::iter::once(header.to_owned())
        .chain(rows)
        .collect::<Vec<_>>()
        .join("\n")
}
