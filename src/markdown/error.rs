use thiserror::Error;

use crate::domain::{EdgeId, EntityId, ScenarioId};

/// Source-aware failures returned by bounded Markdown parsing and rendering.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum MarkdownError {
    /// The complete document exceeds the parser's configured byte limit.
    #[error("{path}: document exceeds the {maximum} byte limit")]
    DocumentTooLarge {
        /// Path supplied by the import caller.
        path: String,
        /// Maximum accepted UTF-8 byte length.
        maximum: usize,
    },
    /// The document contains carriage returns instead of canonical LF endings.
    #[error("{0}: Markdown documents must use LF line endings")]
    NonCanonicalLineEndings(String),
    /// Opening or closing `---` delimiters are absent or misplaced.
    #[error("{0}: expected YAML frontmatter delimited by exact `---` lines")]
    MissingFrontmatter(String),
    /// The YAML frontmatter exceeds its independent byte limit.
    #[error("{path}: frontmatter exceeds the {maximum} byte limit")]
    FrontmatterTooLarge {
        /// Path supplied by the import caller.
        path: String,
        /// Maximum accepted YAML byte length.
        maximum: usize,
    },
    /// Structured YAML decoding failed at the reported source position.
    #[error("{path}:{line}:{column}: invalid YAML: {message}")]
    InvalidYaml {
        /// Path supplied by the import caller.
        path: String,
        /// One-based YAML line, including the opening delimiter offset.
        line: usize,
        /// One-based YAML column.
        column: usize,
        /// Parser diagnostic.
        message: String,
    },
    /// The document uses a schema version this binary cannot interpret safely.
    #[error("{path}: unsupported Markdown schema version {version}")]
    UnsupportedSchema {
        /// Path supplied by the import caller.
        path: String,
        /// Version declared in frontmatter.
        version: u32,
    },
    /// A node's persisted normalized name disagrees with its semantic name.
    #[error("{path}: node {node} has a non-canonical normalized name")]
    InvalidNodeName {
        /// Source path.
        path: String,
        /// Affected project-local node ID.
        node: EntityId,
    },
    /// An edge in an entity file names a different source node.
    #[error("{path}: outgoing edge {edge} does not start at node {node}")]
    ForeignOutgoingEdge {
        /// Source path.
        path: String,
        /// Entity document owner.
        node: EntityId,
        /// Invalid outgoing edge identity.
        edge: EdgeId,
    },
    /// Two outgoing edge records have the same canonical tuple identity.
    #[error("{path}: duplicate outgoing edge {edge}")]
    DuplicateEdge {
        /// Source path.
        path: String,
        /// Repeated edge identity.
        edge: EdgeId,
    },
    /// An outgoing edge payload is illegal for its declared endpoint kinds.
    #[error("{path}: invalid outgoing edge {edge}: {message}")]
    InvalidEdge {
        /// Source path.
        path: String,
        /// Invalid edge identity.
        edge: EdgeId,
        /// Domain validation diagnostic.
        message: String,
    },
    /// A scenario document violates aggregate-local validation rules.
    #[error("{path}: invalid scenario {scenario}: {message}")]
    InvalidScenario {
        /// Source path or render boundary.
        path: String,
        /// Affected project-local scenario ID.
        scenario: ScenarioId,
        /// Domain validation diagnostic.
        message: String,
    },
    /// A project dependence document violates project, membership, or matrix rules.
    #[error("{path}: invalid project dependence: {message}")]
    InvalidDependence {
        /// Source path or render boundary.
        path: String,
        /// Domain validation diagnostic.
        message: String,
    },
    /// YAML serialization failed for an already validated document.
    #[error("could not render Markdown frontmatter: {0}")]
    Render(String),
}
