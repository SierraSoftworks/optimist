use thiserror::Error;

use crate::project_yaml::{ImportError, YamlError};
use crate::{
    domain::{
        AnalysisError, DependenceError, EdgeId, EdgeIdError, EntityId, EstimateAddress,
        EstimateAddressError, MeasurementCalibrationError, NodeError, NodeKind, ObservationError,
        ProjectId, QuantityError, ScenarioAnalysisError, ScenarioError, ScenarioId,
    },
    store::RepositoryError,
};

use super::{AggregateUpdateError, CommandBatchError, EstimateCommandError, EvidenceCommandError};

/// Failures which prevent project lifecycle operations from completing.
///
/// HTTP handlers map these variants to stable status/code/advice responses, while
/// CLI callers wrap them in `human_errors` at the process boundary.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ProjectError {
    /// The proposed name contains no text after normalization.
    #[error("a project name cannot be empty")]
    EmptyName,
    /// Another project already claims the normalized form of this name.
    #[error("a project named {0:?} already exists")]
    DuplicateName(String),
    /// No project exists for the requested server-local ID.
    #[error("project {0} does not exist")]
    NotFound(ProjectId),
    /// The server's monotonic project-ID counter cannot allocate another value.
    #[error("the server has exhausted its project identifier space")]
    IdentifierSpaceExhausted,
    /// A command was based on an older project revision than the current graph.
    #[error("project revision conflict: expected {expected}, current {current}")]
    RevisionConflict {
        /// Revision supplied by the client before it prepared the mutation.
        expected: u64,
        /// Revision currently stored by the serialized project executor.
        current: u64,
    },
    /// An atomic command batch is empty, oversized, conflicting, or cannot be compensated.
    #[error(transparent)]
    CommandBatch(#[from] CommandBatchError),
    /// The project revision counter cannot represent another committed mutation.
    #[error("project {0} has exhausted its revision space")]
    RevisionSpaceExhausted(ProjectId),
    /// The independent graph revision counter cannot represent another mutation.
    #[error("project {0} has exhausted its graph revision space")]
    GraphRevisionSpaceExhausted(ProjectId),
    /// A replay cursor is newer than the project's current revision.
    #[error("change replay revision {requested} exceeds current project revision {current}")]
    InvalidReplayRevision {
        /// Exclusive lower revision requested by the client.
        requested: u64,
        /// Current committed project revision.
        current: u64,
    },
    /// Requested replay predates the earliest retained ChangeSet.
    #[error(
        "change history starts at revision {available_after}; requested replay after {requested}"
    )]
    ChangeHistoryGap {
        /// Cursor supplied by the caller.
        requested: u64,
        /// Earliest cursor from which complete replay is available.
        available_after: u64,
    },
    /// The requested node aggregate failed local construction validation.
    #[error(transparent)]
    Node(#[from] NodeError),
    /// An external edge ID does not use the canonical tuple representation.
    #[error(transparent)]
    EdgeId(#[from] EdgeIdError),
    /// An external estimate address does not use the canonical tagged path grammar.
    #[error(transparent)]
    EstimateAddress(#[from] EstimateAddressError),
    /// The selected edge does not own a measurement observation series.
    #[error("edge {0} is not a measurement edge")]
    NotMeasurementEdge(EdgeId),
    /// A reading's unit disagrees with the source metric definition.
    #[error("observation unit {actual:?} does not match metric unit {expected:?}")]
    ObservationUnitMismatch {
        /// Unit declared by the source metric node.
        expected: String,
        /// Unit supplied with the new observation.
        actual: String,
    },
    /// Only factors and outcomes may replace standardized state with native state.
    #[error("node {0} cannot own native quantity state")]
    NativeStateUnsupported(EntityId),
    /// Existing state estimates must be removed before changing their quantity.
    #[error("node {0} already has state estimates")]
    StateEstimatesAlreadyExist(EntityId),
    /// A native state quantity or estimate is internally inconsistent.
    #[error(transparent)]
    Quantity(#[from] QuantityError),
    /// A native causal response does not use the units declared by its endpoints.
    #[error("edge {edge} response units do not match its endpoints")]
    CausalResponseUnitMismatch {
        /// Canonical edge whose response is inconsistent.
        edge: EdgeId,
        /// Unit declared by the source node.
        expected_source: crate::domain::Unit,
        /// Source unit declared by the response.
        actual_source: crate::domain::Unit,
        /// Unit declared by the destination node.
        expected_destination: crate::domain::Unit,
        /// Destination unit declared by the response.
        actual_destination: crate::domain::Unit,
    },
    /// A metric needs canonical unit terms before it can participate causally.
    #[error("node {0} requires a canonical quantity dimension for causal modelling")]
    MissingQuantityDimension(EntityId),
    /// The edge aggregate cannot represent another revision.
    #[error("edge {0} has exhausted its revision space")]
    EdgeRevisionSpaceExhausted(EdgeId),
    /// Revision-checked node or edge presentation metadata update failed.
    #[error(transparent)]
    AggregateUpdate(#[from] AggregateUpdateError),
    /// Observation validation or immutable correction semantics failed.
    #[error(transparent)]
    Observation(#[from] ObservationError),
    /// A measurement calibration has invalid anchors or conflicts with its polarity.
    #[error(transparent)]
    MeasurementCalibration(#[from] MeasurementCalibrationError),
    /// Node-owned evidence authoring or deletion failed.
    #[error(transparent)]
    EvidenceCommand(#[from] EvidenceCommandError),
    /// Scenario aggregate-local validation failed.
    #[error(transparent)]
    Scenario(#[from] ScenarioError),
    /// The requested scenario document does not exist in this project.
    #[error("scenario {0} does not exist")]
    ScenarioNotFound(ScenarioId),
    /// A scenario update or delete was based on an older document revision.
    #[error("scenario {id} revision conflict: expected {expected}, current {current}")]
    ScenarioRevisionConflict {
        /// Project-local scenario document ID.
        id: ScenarioId,
        /// Revision supplied by the caller.
        expected: u64,
        /// Revision currently stored by the project.
        current: u64,
    },
    /// A scenario's document revision cannot represent another update.
    #[error("scenario {0} has exhausted its revision space")]
    ScenarioRevisionSpaceExhausted(ScenarioId),
    /// A scenario's independent project-local ID counter is exhausted.
    #[error("project {0} has exhausted its scenario identifier space")]
    ScenarioIdentifierSpaceExhausted(ProjectId),
    /// A scenario reference points at a missing or wrong-kind graph entity.
    #[error("scenario reference {id} must identify a {expected:?}, found {actual:?}")]
    InvalidScenarioReference {
        /// Project-local graph entity ID referenced by the scenario.
        id: EntityId,
        /// Required structural entity kind.
        expected: NodeKind,
        /// Stored kind, or `None` when no such entity exists.
        actual: Option<NodeKind>,
    },
    /// Project dependence matrix or membership validation failed.
    #[error(transparent)]
    Dependence(#[from] DependenceError),
    /// No dependence document exists for a show or remove operation.
    #[error("project {0} has no dependence document")]
    DependenceNotFound(ProjectId),
    /// A dependence replacement/removal used an older document revision.
    #[error("dependence revision conflict: expected {expected}, current {current}")]
    DependenceRevisionConflict {
        /// Revision supplied by the caller.
        expected: u64,
        /// Revision currently stored in the project.
        current: u64,
    },
    /// The dependence document revision cannot represent another replacement.
    #[error("project {0} has exhausted its dependence revision space")]
    DependenceRevisionSpaceExhausted(ProjectId),
    /// An address does not resolve to an estimate embedded in the project graph.
    #[error("dependence address {0} does not identify a stored estimate")]
    MissingEstimateAddress(EstimateAddress),
    /// Primitive estimate authoring or lookup validation failed.
    #[error(transparent)]
    EstimateCommand(#[from] EstimateCommandError),
    /// Exact structural analysis input or bounds are invalid.
    #[error(transparent)]
    Analysis(#[from] AnalysisError),
    /// Finite-horizon scenario propagation inputs or assumptions are invalid.
    #[error(transparent)]
    ScenarioAnalysis(#[from] ScenarioAnalysisError),
    /// A canonical YAML project document could not be parsed or rendered.
    #[error(transparent)]
    Yaml(#[from] YamlError),
    /// A complete YAML project structure failed cross-document validation.
    #[error(transparent)]
    Import(#[from] ImportError),
    /// An archive replacement was requested without explicit destructive confirmation.
    #[error("replacing project {0} requires both replace and yes confirmation")]
    ReplaceConfirmationRequired(ProjectId),
    /// A non-replacement import attempted to restore an ID which already exists.
    #[error("project {0} already exists; explicit replacement is required")]
    ImportProjectExists(ProjectId),
    /// A persisted project path or document identity is invalid.
    #[error("invalid persisted project path {0:?}")]
    InvalidArchivePath(String),
    /// The archive exceeds the bounded project file count.
    #[error("project archive exceeds the 10,001 file limit")]
    ArchiveTooManyFiles,
    /// Canonical YAML content exceeds the bounded archive size.
    #[error("project archive exceeds the 32 MiB content limit")]
    ArchiveTooLarge,
    /// Creating or accessing the project's isolated graph repository failed.
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}
