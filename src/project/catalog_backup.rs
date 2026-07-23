use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::ProjectId;

use super::{CatalogPersistenceError, Project, ProjectArchiveSummary, ProjectError};

/// Metadata describing one immutable full-catalog backup.
///
/// Backups contain the complete catalog, including canonical project archives,
/// replay history, and command retry results. The opaque ID is suitable for a
/// later restore request.
///
/// ```
/// # use optimist::project::CatalogBackup;
/// let backup: CatalogBackup = serde_json::from_str(
///     r#"{"id":"00000000-0000-4000-8000-000000000001","created_unix_ms":1,"size_bytes":128,"projects":[]}"#,
/// )?;
/// assert_eq!(backup.size_bytes, 128);
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CatalogBackup {
    /// Opaque immutable backup identifier.
    pub id: Uuid,
    /// Creation time as Unix epoch milliseconds.
    pub created_unix_ms: u64,
    /// Encoded catalog size in bytes.
    pub size_bytes: u64,
    /// Projects and revisions captured by this backup.
    pub projects: Vec<Project>,
}

/// Result of restoring a full-catalog backup.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CatalogRestore {
    /// Backup selected for restoration.
    pub restored: CatalogBackup,
    /// Automatic backup of the catalog which existed immediately before restore.
    pub safety_backup: CatalogBackup,
    /// Projects now active after restoration.
    pub projects: Vec<Project>,
}

/// Metadata for one immutable canonical project snapshot.
///
/// Repeating snapshot creation at the same revision is idempotent: the same
/// canonical archive is retained rather than overwritten.
///
/// ```
/// # use optimist::project::ProjectSnapshot;
/// let snapshot: ProjectSnapshot = serde_json::from_str(
///     r#"{"project":"A","revision":4,"size_bytes":256,"summary":{"entities":2,"edges":1,"scenarios":0}}"#,
/// )?;
/// assert_eq!(snapshot.revision, 4);
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectSnapshot {
    /// Project represented by the snapshot.
    pub project: ProjectId,
    /// Immutable project revision represented by the archive.
    pub revision: u64,
    /// Encoded archive size in bytes.
    pub size_bytes: u64,
    /// Aggregate counts retained by the canonical archive.
    pub summary: ProjectArchiveSummary,
}

/// Failures while creating, listing, reading, or restoring immutable backups.
#[derive(Debug, Error)]
pub enum BackupError {
    /// Backup APIs require a server configured with `--data-dir` persistence.
    #[error("backup storage is unavailable for this in-memory server")]
    Unavailable,
    /// Explicit confirmation is required before replacing the complete catalog.
    #[error("restoring a catalog backup requires yes confirmation")]
    ConfirmationRequired,
    /// The requested immutable backup does not exist.
    #[error("catalog backup {0} does not exist")]
    BackupNotFound(Uuid),
    /// The requested immutable project snapshot does not exist.
    #[error("project {project} has no snapshot at revision {revision}")]
    SnapshotNotFound {
        /// Project whose snapshot was requested.
        project: ProjectId,
        /// Requested immutable revision.
        revision: u64,
    },
    /// A backup or snapshot path could not be read or atomically published.
    #[error("could not access backup path {path}")]
    Io {
        /// Filesystem path involved in the failure.
        path: PathBuf,
        /// Underlying filesystem failure.
        #[source]
        source: std::io::Error,
    },
    /// Backup metadata or a project archive could not be decoded.
    #[error("backup file {path} is not valid JSON")]
    Json {
        /// Invalid backup or snapshot path.
        path: PathBuf,
        /// JSON decoding failure.
        #[source]
        source: serde_json::Error,
    },
    /// Catalog snapshot loading or integrity validation failed.
    #[error(transparent)]
    Catalog(#[from] CatalogPersistenceError),
    /// Canonical project export or validation failed.
    #[error(transparent)]
    Project(#[from] ProjectError),
}
