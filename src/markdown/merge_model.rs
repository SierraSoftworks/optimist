use crate::domain::ProjectId;

/// Safe action proposed for one imported aggregate without applying it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MergeAction {
    /// The imported aggregate does not exist in the current snapshot.
    Create,
    /// The aggregate changed from the same base and may be replaced.
    Update,
    /// Imported and current semantic content are equal.
    Unchanged,
    /// Applying the imported content could overwrite concurrent work.
    Conflict(MergeConflict),
}

/// Reason a Markdown aggregate cannot be merged automatically.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MergeConflict {
    /// The import belongs to a different isolated project.
    DifferentProject {
        /// Project selected as the merge target.
        current: ProjectId,
        /// Project declared by the imported `_project.md`.
        imported: ProjectId,
    },
    /// Changed content was exported from a stale project revision.
    BaseRevision {
        /// Current project revision.
        current: u64,
        /// Base revision declared by the import.
        imported: u64,
    },
    /// An aggregate changed since the imported version was exported.
    AggregateRevision {
        /// Current node or scenario revision.
        current: u64,
        /// Revision retained by the imported aggregate.
        imported: u64,
    },
}
