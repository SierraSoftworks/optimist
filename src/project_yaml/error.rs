use thiserror::Error;

use crate::domain::{EdgeId, EntityId, ScenarioId};

/// Source-aware failures returned by bounded YAML project parsing and rendering.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum YamlError {
    /// A document exceeds the bounded parser input size.
    #[error("{path}: YAML document exceeds the {maximum} byte limit")]
    DocumentTooLarge {
        /// Project-relative source path.
        path: String,
        /// Maximum accepted UTF-8 byte count.
        maximum: usize,
    },
    /// A document uses CR or CRLF instead of canonical LF line endings.
    #[error("{0}: YAML documents must use LF line endings")]
    NonCanonicalLineEndings(String),
    /// YAML syntax or shape is invalid.
    #[error("{path}: invalid YAML: {message}")]
    InvalidYaml {
        /// Project-relative source path.
        path: String,
        /// Parser diagnostic including source location when available.
        message: String,
    },
    /// The document declares an unsupported schema version.
    #[error("{path}: unsupported YAML project schema version {version}")]
    UnsupportedSchema {
        /// Project-relative source path.
        path: String,
        /// Unsupported version supplied by the document.
        version: u32,
    },
    /// Project dependence is structurally invalid.
    #[error("{path}: invalid project dependence: {message}")]
    InvalidDependence {
        /// Project document path.
        path: String,
        /// Dependence validation diagnostic.
        message: String,
    },
    /// A node contains invalid native-state data.
    #[error("{path}: invalid node {node}: {message}")]
    InvalidNode {
        /// Entity document path.
        path: String,
        /// Affected node.
        node: EntityId,
        /// Node validation diagnostic.
        message: String,
    },
    /// A node's normalized lookup key does not match its authored name.
    #[error("{path}: node {node} has a non-canonical normalized name")]
    InvalidNodeName {
        /// Entity document path.
        path: String,
        /// Affected node.
        node: EntityId,
    },
    /// An entity file contains an edge owned by another source node.
    #[error("{path}: node {node} cannot own outgoing edge {edge}")]
    ForeignOutgoingEdge {
        /// Entity document path.
        path: String,
        /// Node owning the YAML file.
        node: EntityId,
        /// Edge with a different source.
        edge: EdgeId,
    },
    /// An entity file repeats one canonical edge identity.
    #[error("{path}: duplicate outgoing edge {edge}")]
    DuplicateEdge {
        /// Entity document path.
        path: String,
        /// Repeated edge identity.
        edge: EdgeId,
    },
    /// An edge fails aggregate-local semantic validation.
    #[error("{path}: invalid edge {edge}: {message}")]
    InvalidEdge {
        /// Entity document path.
        path: String,
        /// Affected edge identity.
        edge: EdgeId,
        /// Edge validation diagnostic.
        message: String,
    },
    /// A scenario fails aggregate-local validation.
    #[error("{path}: invalid scenario {scenario}: {message}")]
    InvalidScenario {
        /// Scenario document path.
        path: String,
        /// Affected scenario.
        scenario: ScenarioId,
        /// Scenario validation diagnostic.
        message: String,
    },
    /// A validated document could not be rendered as YAML.
    #[error("could not render YAML project document: {0}")]
    Render(String),
}
