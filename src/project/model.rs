use serde::{Deserialize, Serialize};

use crate::domain::ProjectId;

/// Public metadata for one isolated causal graph.
///
/// The ID selects API/storage scope, while the name is human-facing. Revision is
/// returned with results so clients can later reject stale project-document edits.
///
/// ```
/// use optimist::project::ProjectCatalog;
/// let mut catalog = ProjectCatalog::new();
/// let project = catalog.create("Platform reliability".to_owned())?;
/// assert_eq!(project.id.as_str(), "A");
/// # Ok::<(), optimist::project::ProjectError>(())
/// ```
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Project {
    /// Server-local short identifier used in API paths and CLI selection.
    pub id: ProjectId,
    /// Human-facing project name, unique under canonical case-insensitive matching.
    pub name: String,
    /// Optimistic-concurrency revision for project-level documents and settings.
    pub revision: u64,
}

/// Request body used to create an isolated project graph.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CreateProject {
    /// Human-facing name from which uniqueness is checked; the server allocates the ID.
    pub name: String,
}
