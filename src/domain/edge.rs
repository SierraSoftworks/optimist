use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{Duration, EntityId, Estimate, NodeKind, SignedInfluence};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Contributes,
    Measures,
    Changes,
    Requires,
    PartOf,
    Blocks,
    ConflictsWith,
    SynergizesWith,
}

impl EdgeKind {
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

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EdgeId {
    pub source: EntityId,
    pub kind: EdgeKind,
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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CausalEffect {
    pub effect: Estimate<SignedInfluence>,
    pub lag: Option<Estimate<Duration>>,
    pub mechanism: String,
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Observation {
    pub id: u64,
    pub revision: u64,
    pub value: f64,
    pub unit: String,
    pub observed_at: String,
    pub source: String,
    pub measurement_standard_deviation: Option<f64>,
    pub supersedes: Option<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementPolarity {
    HigherIsBetter,
    LowerIsBetter,
    TargetRange,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Measurement {
    pub polarity: MeasurementPolarity,
    #[serde(default)]
    pub observations: Vec<Observation>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Requirement {
    pub hard: bool,
    pub satisfaction_threshold: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BlockingEffect {
    pub degree: Estimate<SignedInfluence>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "properties", rename_all = "snake_case")]
pub enum EdgePayload {
    Contributes(CausalEffect),
    Measures(Measurement),
    Changes(CausalEffect),
    Requires(Requirement),
    PartOf,
    Blocks(BlockingEffect),
    ConflictsWith,
    SynergizesWith,
}

impl EdgePayload {
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

#[derive(Clone, Debug, Error, PartialEq)]
pub enum EdgeError {
    #[error("{kind:?} cannot connect {source_kind:?} to {destination_kind:?}")]
    InvalidEndpoints {
        kind: EdgeKind,
        source_kind: NodeKind,
        destination_kind: NodeKind,
    },
    #[error("a symmetric relationship cannot connect a node to itself")]
    SymmetricSelfEdge,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Edge {
    pub source: EntityId,
    pub source_kind: NodeKind,
    pub destination: EntityId,
    pub destination_kind: NodeKind,
    pub revision: u64,
    pub payload: EdgePayload,
}

impl Edge {
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
