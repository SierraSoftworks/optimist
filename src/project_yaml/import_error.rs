use thiserror::Error;

use crate::domain::{EdgeId, EntityId, EstimateAddress, NodeKind, ScenarioId};

/// Source-aware failures returned while validating a complete YAML project.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ImportError {
    /// Two imported documents use the same relative source path.
    #[error("{path}: duplicate imported document path")]
    DuplicatePath {
        /// Repeated project-relative source path.
        path: String,
    },
    /// Two entity documents declare the same project-local node identity.
    #[error("{path}: node {node} is already declared by {first_path}")]
    DuplicateNode {
        /// Path of the duplicate declaration.
        path: String,
        /// Path of the first declaration.
        first_path: String,
        /// Repeated project-local node identity.
        node: EntityId,
    },
    /// Two entity documents claim the same normalized name or alias.
    #[error("{path}: node name or alias `{name}` is already declared by {first_path}")]
    DuplicateNodeName {
        /// Path containing the duplicate name or alias.
        path: String,
        /// Path which first claimed the normalized name.
        first_path: String,
        /// Conflicting normalized name.
        name: String,
    },
    /// Two scenario documents declare the same project-local identity.
    #[error("{path}: scenario {scenario} is already declared by {first_path}")]
    DuplicateScenario {
        /// Path of the duplicate declaration.
        path: String,
        /// Path of the first declaration.
        first_path: String,
        /// Repeated project-local scenario identity.
        scenario: ScenarioId,
    },
    /// Two scenario documents claim the same normalized project-local name.
    #[error("{path}: scenario name `{name}` is already declared by {first_path}")]
    DuplicateScenarioName {
        /// Path containing the duplicate scenario name.
        path: String,
        /// Path which first claimed the normalized name.
        first_path: String,
        /// Conflicting normalized scenario name.
        name: String,
    },
    /// A document was exported from a different project revision.
    #[error(
        "{path}: base project revision {actual} does not match _project.yaml revision {expected}"
    )]
    InconsistentBaseRevision {
        /// Path exported from the inconsistent revision.
        path: String,
        /// Revision declared by `_project.yaml`.
        expected: u64,
        /// Revision declared by this document.
        actual: u64,
    },
    /// An outgoing edge references an entity absent from the imported project.
    #[error("{path}: outgoing edge {edge} references missing node {node}")]
    MissingEdgeEndpoint {
        /// Entity document containing the edge.
        path: String,
        /// Affected canonical edge identity.
        edge: EdgeId,
        /// Missing endpoint identity.
        node: EntityId,
    },
    /// An edge's declared endpoint kind disagrees with the imported node.
    #[error(
        "{path}: outgoing edge {edge} declares {declared:?} for node {node}, but the node is {actual:?}"
    )]
    EdgeEndpointKindMismatch {
        /// Entity document containing the edge.
        path: String,
        /// Affected canonical edge identity.
        edge: EdgeId,
        /// Endpoint whose kind is inconsistent.
        node: EntityId,
        /// Kind declared by the edge record.
        declared: NodeKind,
        /// Kind implied by the resolved node payload.
        actual: NodeKind,
    },
    /// A scenario reference is missing or resolves to the wrong node kind.
    #[error(
        "{path}: scenario {scenario} requires node {node} to be {expected:?}, found {actual:?}"
    )]
    InvalidScenarioReference {
        /// Scenario document containing the reference.
        path: String,
        /// Affected scenario identity.
        scenario: ScenarioId,
        /// Referenced project-local node identity.
        node: EntityId,
        /// Structural node kind required by this scenario field.
        expected: NodeKind,
        /// Resolved kind, or `None` when the node is absent.
        actual: Option<NodeKind>,
    },
    /// A dependence member does not resolve to an embedded estimate in the import.
    #[error("{path}: dependence address {address} does not resolve to an imported estimate")]
    MissingDependenceEstimate {
        /// Project document containing the dependence model.
        path: String,
        /// Unresolved project-scoped estimate address.
        address: EstimateAddress,
    },
}
