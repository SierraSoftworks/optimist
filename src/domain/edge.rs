use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{Duration, EntityId, Estimate, NodeKind, SignedInfluence};

/// Structural relationship kinds supported by the causal graph.
///
/// Direction is semantically meaningful except for the two intervention interaction
/// kinds, which [`Edge::new`] canonicalizes by endpoint ID.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// A signed causal effect flowing from a factor/outcome to another subject.
    Contributes,
    /// A metric observing a specific factor or outcome; owns that reading series.
    Measures,
    /// An intervention's expected signed effect on a factor.
    Changes,
    /// A hard or soft prerequisite from a factor/intervention to another.
    Requires,
    /// Non-causal decomposition of a factor into a parent factor.
    PartOf,
    /// A factor preventing or reducing another factor/intervention.
    Blocks,
    /// Symmetric incompatibility between two intervention choices.
    ConflictsWith,
    /// Symmetric beneficial interaction between two intervention choices.
    SynergizesWith,
}

impl EdgeKind {
    /// Returns the stable delimiter-safe token used in IndraDB and external edge IDs.
    ///
    /// ```
    /// use optimist::domain::EdgeKind;
    /// assert_eq!(EdgeKind::Contributes.token(), "contrib");
    /// ```
    pub const fn token(self) -> &'static str {
        match self {
            Self::Contributes => "contrib",
            Self::Measures => "measures",
            Self::Changes => "changes",
            Self::Requires => "requires",
            Self::PartOf => "part-of",
            Self::Blocks => "blocks",
            Self::ConflictsWith => "conflicts",
            Self::SynergizesWith => "synergizes",
        }
    }

    const fn is_symmetric(self) -> bool {
        matches!(self, Self::ConflictsWith | Self::SynergizesWith)
    }
}

/// Canonical identity of an edge within one project.
///
/// Edge uniqueness is the tuple `(source, kind, destination)`, rendered as a
/// compact string such as `A-requires-B` for agent and Markdown references.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EdgeId {
    /// Outbound entity from which the relationship originates.
    pub source: EntityId,
    /// Semantic relationship kind.
    pub kind: EdgeKind,
    /// Inbound entity to which the relationship points.
    pub destination: EntityId,
}

impl fmt::Display for EdgeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}-{}-{}",
            self.source,
            self.kind.token(),
            self.destination
        )
    }
}

/// Uncertain local causal effect embedded in a `contributes` or `changes` edge.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CausalEffect {
    /// Signed probability distribution for direction and normalized local strength.
    pub effect: Estimate<SignedInfluence>,
    /// Optional non-negative delay before the effect reaches its destination.
    pub lag: Option<Estimate<Duration>>,
    /// Markdown explanation of the causal mechanism, boundaries, and assumptions.
    pub mechanism: String,
    /// Aggregate-local evidence references supporting this relationship.
    #[serde(default)]
    pub evidence: Vec<String>,
}

/// One immutable quantitative reading embedded in a [`Measurement`] edge payload.
///
/// Corrections append a new observation and set [`Observation::supersedes`] rather
/// than rewriting evidence used by earlier analyses.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Observation {
    /// Measurement-edge-local identifier used for correction references.
    pub id: u64,
    /// Optimistic-concurrency revision of this reading.
    pub revision: u64,
    /// Observed numeric value expressed in [`Observation::unit`].
    pub value: f64,
    /// Unit recorded at collection time and checked against the metric definition.
    pub unit: String,
    /// RFC 3339 timestamp or project-approved temporal representation.
    pub observed_at: String,
    /// Person, system, query, or citation which produced the reading.
    pub source: String,
    /// Known measurement-error standard deviation; `None` means unknown, not zero.
    pub measurement_standard_deviation: Option<f64>,
    /// Earlier observation replaced by this correction, if any.
    pub supersedes: Option<u64>,
}

/// Maps metric movement to desirability for its measured subject.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementPolarity {
    /// Larger readings indicate a better subject state.
    HigherIsBetter,
    /// Smaller readings indicate a better subject state.
    LowerIsBetter,
    /// Readings are best inside a separately configured interval.
    TargetRange,
}

/// Metric-to-subject measurement data owned by a `measures` edge.
///
/// Edge ownership distinguishes, for example, one latency metric measuring two
/// services with separate histories without introducing observation vertices.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Measurement {
    /// Interpretation of movement when mapping readings to subject state.
    pub polarity: MeasurementPolarity,
    /// Append-only readings for this exact metric/subject pair.
    #[serde(default)]
    pub observations: Vec<Observation>,
}

/// Prerequisite semantics embedded in a `requires` edge.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Requirement {
    /// Whether an unsatisfied prerequisite makes its source infeasible.
    pub hard: bool,
    /// Optional normalized state at which the prerequisite is considered satisfied.
    pub satisfaction_threshold: Option<f64>,
}

/// Uncertain blocking strength embedded in a `blocks` edge.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BlockingEffect {
    /// Signed normalized effect; negative values normally represent inhibition.
    pub degree: Estimate<SignedInfluence>,
}

/// Type-safe payload defining an edge's semantics and embedded owned values.
///
/// ```
/// use optimist::domain::{EdgePayload, Requirement};
/// let payload = EdgePayload::Requires(Requirement {
///     hard: true,
///     satisfaction_threshold: None,
/// });
/// ```
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "properties", rename_all = "snake_case")]
pub enum EdgePayload {
    /// Signed causal influence between system conditions or results.
    Contributes(CausalEffect),
    /// Measurement definition and readings for a metric/subject pair.
    Measures(Measurement),
    /// Expected intervention effect on a factor.
    Changes(CausalEffect),
    /// Hard or soft prerequisite semantics.
    Requires(Requirement),
    /// Non-causal factor hierarchy with no additional fields.
    PartOf,
    /// Blocking effect exerted by the source factor.
    Blocks(BlockingEffect),
    /// Symmetric intervention incompatibility with no additional fields yet.
    ConflictsWith,
    /// Symmetric intervention synergy with no additional fields yet.
    SynergizesWith,
}

impl EdgePayload {
    /// Returns the structural relationship kind implied by this payload variant.
    pub const fn kind(&self) -> EdgeKind {
        match self {
            Self::Contributes(_) => EdgeKind::Contributes,
            Self::Measures(_) => EdgeKind::Measures,
            Self::Changes(_) => EdgeKind::Changes,
            Self::Requires(_) => EdgeKind::Requires,
            Self::PartOf => EdgeKind::PartOf,
            Self::Blocks(_) => EdgeKind::Blocks,
            Self::ConflictsWith => EdgeKind::ConflictsWith,
            Self::SynergizesWith => EdgeKind::SynergizesWith,
        }
    }
}

/// Validation failures returned when edge semantics do not match their endpoints.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum EdgeError {
    /// The relationship kind is not legal between the declared node kinds.
    #[error("{kind:?} cannot connect {source_kind:?} to {destination_kind:?}")]
    InvalidEndpoints {
        /// Relationship being validated.
        kind: EdgeKind,
        /// Declared kind of the outbound endpoint.
        source_kind: NodeKind,
        /// Declared kind of the inbound endpoint.
        destination_kind: NodeKind,
    },
    /// A symmetric interaction attempted to connect an intervention to itself.
    #[error("a symmetric relationship cannot connect a node to itself")]
    SymmetricSelfEdge,
}

/// A validated structural graph relationship with an embedded typed payload.
///
/// Always construct edges with [`Edge::new`]; it enforces the endpoint matrix and
/// canonicalizes symmetric relationships before identity or persistence is derived.
///
/// ```
/// use optimist::domain::{
///     Edge, EdgePayload, EntityId, NodeKind, Requirement,
/// };
///
/// let edge = Edge::new(
///     EntityId::new(0),
///     NodeKind::Factor,
///     EntityId::new(1),
///     NodeKind::Factor,
///     EdgePayload::Requires(Requirement {
///         hard: true,
///         satisfaction_threshold: None,
///     }),
/// )?;
/// assert_eq!(edge.id().to_string(), "A-requires-B");
/// # Ok::<(), optimist::domain::EdgeError>(())
/// ```
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Edge {
    /// Outbound project-local entity ID.
    pub source: EntityId,
    /// Declared outbound kind, rechecked against the stored source node on insertion.
    pub source_kind: NodeKind,
    /// Inbound project-local entity ID.
    pub destination: EntityId,
    /// Declared inbound kind, rechecked against the stored destination node.
    pub destination_kind: NodeKind,
    /// Optimistic-concurrency revision of the edge aggregate.
    pub revision: u64,
    /// Strongly typed relationship data and embedded owned values.
    pub payload: EdgePayload,
}

impl Edge {
    /// Validates and constructs a canonical edge.
    ///
    /// Directed relationships retain caller order. `conflicts_with` and
    /// `synergizes_with` order endpoints by ID so both input orders share one key.
    pub fn new(
        mut source: EntityId,
        mut source_kind: NodeKind,
        mut destination: EntityId,
        mut destination_kind: NodeKind,
        payload: EdgePayload,
    ) -> Result<Self, EdgeError> {
        let kind = payload.kind();
        if !endpoints_are_valid(kind, source_kind, destination_kind) {
            return Err(EdgeError::InvalidEndpoints {
                kind,
                source_kind,
                destination_kind,
            });
        }
        if kind.is_symmetric() && source == destination {
            return Err(EdgeError::SymmetricSelfEdge);
        }
        if kind.is_symmetric() && source > destination {
            std::mem::swap(&mut source, &mut destination);
            std::mem::swap(&mut source_kind, &mut destination_kind);
        }

        Ok(Self {
            source,
            source_kind,
            destination,
            destination_kind,
            revision: 0,
            payload,
        })
    }

    /// Returns the canonical tuple identity used by repositories and external APIs.
    pub fn id(&self) -> EdgeId {
        EdgeId {
            source: self.source,
            kind: self.payload.kind(),
            destination: self.destination,
        }
    }
}

const fn endpoints_are_valid(kind: EdgeKind, source: NodeKind, destination: NodeKind) -> bool {
    match kind {
        EdgeKind::Contributes => {
            matches!(source, NodeKind::Factor | NodeKind::Outcome)
                && matches!(
                    destination,
                    NodeKind::Factor | NodeKind::Metric | NodeKind::Outcome
                )
        }
        EdgeKind::Measures => {
            matches!(source, NodeKind::Metric)
                && matches!(destination, NodeKind::Factor | NodeKind::Outcome)
        }
        EdgeKind::Changes => {
            matches!(source, NodeKind::Intervention) && matches!(destination, NodeKind::Factor)
        }
        EdgeKind::Requires => {
            matches!(source, NodeKind::Factor | NodeKind::Intervention)
                && matches!(destination, NodeKind::Factor | NodeKind::Intervention)
        }
        EdgeKind::PartOf => {
            matches!(source, NodeKind::Factor) && matches!(destination, NodeKind::Factor)
        }
        EdgeKind::Blocks => {
            matches!(source, NodeKind::Factor)
                && matches!(destination, NodeKind::Factor | NodeKind::Intervention)
        }
        EdgeKind::ConflictsWith | EdgeKind::SynergizesWith => {
            matches!(source, NodeKind::Intervention)
                && matches!(destination, NodeKind::Intervention)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Edge, EdgeError, EdgePayload, Measurement, MeasurementPolarity};
    use crate::domain::{EntityId, NodeKind};

    #[test]
    fn measurements_are_owned_by_metric_to_subject_edges() {
        let edge = Edge::new(
            EntityId::new(1),
            NodeKind::Metric,
            EntityId::new(2),
            NodeKind::Factor,
            EdgePayload::Measures(Measurement {
                polarity: MeasurementPolarity::HigherIsBetter,
                observations: Vec::new(),
            }),
        )
        .expect("valid measurement edge");

        assert_eq!(edge.id().to_string(), "B-measures-C");
    }

    #[test]
    fn rejects_invalid_measurement_endpoints() {
        let result = Edge::new(
            EntityId::new(1),
            NodeKind::Factor,
            EntityId::new(2),
            NodeKind::Outcome,
            EdgePayload::Measures(Measurement {
                polarity: MeasurementPolarity::HigherIsBetter,
                observations: Vec::new(),
            }),
        );

        assert!(matches!(result, Err(EdgeError::InvalidEndpoints { .. })));
    }

    #[test]
    fn symmetric_edges_have_one_canonical_identity() {
        let edge = Edge::new(
            EntityId::new(10),
            NodeKind::Intervention,
            EntityId::new(3),
            NodeKind::Intervention,
            EdgePayload::ConflictsWith,
        )
        .expect("valid conflict edge");

        assert_eq!(edge.source, EntityId::new(3));
        assert_eq!(edge.destination, EntityId::new(10));
    }
}
