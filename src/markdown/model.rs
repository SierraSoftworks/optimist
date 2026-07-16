use crate::{
    domain::{Edge, Node},
    project::Project,
};

/// Markdown schema version currently accepted and emitted by Optimist.
pub const SCHEMA_VERSION: u32 = 1;

/// Versioned `_project.md` content independent of its filesystem location.
///
/// Frontmatter stores identity and revision metadata; `description` is the Markdown
/// body. Directory import will later combine this value with entity documents.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectDocument {
    /// Schema version used to decode this document.
    pub schema_version: u32,
    /// Project metadata at the exported base revision.
    pub project: Project,
    /// Rich Markdown project rationale and scope.
    pub description: String,
}

/// Versioned entity Markdown content with all relationships owned by its source node.
///
/// The node description is stored only in `Node::description` and rendered as the
/// Markdown body. `outgoing_edges` belong in this file because their source identity
/// is structurally fixed to `node.id`.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityDocument {
    /// Schema version used to decode this document.
    pub schema_version: u32,
    /// Project graph revision from which this entity file was exported.
    pub base_project_revision: u64,
    /// Complete typed node aggregate reconstructed with its Markdown description.
    pub node: Node,
    /// Complete outgoing edge aggregates, sorted by canonical edge ID when rendered.
    pub outgoing_edges: Vec<Edge>,
}
