use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{EdgeId, EdgeKind, EdgePayload, EntityId, MeasurementCalibrationError, NodeKind};

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
    /// A measurement edge contains invalid or polarity-incompatible calibration anchors.
    #[error(transparent)]
    MeasurementCalibration(#[from] MeasurementCalibrationError),
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
    /// Rich Markdown explanation applying to this complete relationship.
    #[serde(default)]
    pub description: String,
    /// Extensible non-structural JSON data owned by this relationship.
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
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
        mut payload: EdgePayload,
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
        if let EdgePayload::Measures(measurement) = &mut payload {
            let calibration = measurement.calibration.take();
            measurement.set_calibration(calibration)?;
        }

        Ok(Self {
            source,
            source_kind,
            destination,
            destination_kind,
            revision: 0,
            description: String::new(),
            metadata: BTreeMap::new(),
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
    use super::{Edge, EdgeError};
    use crate::domain::{
        EdgePayload, EntityId, Measurement, MeasurementCalibration, MeasurementCalibrationError,
        MeasurementPolarity, NodeKind,
    };

    #[test]
    fn measurements_are_owned_by_metric_to_subject_edges() {
        let edge = Edge::new(
            EntityId::new(1),
            NodeKind::Metric,
            EntityId::new(2),
            NodeKind::Factor,
            EdgePayload::Measures(Measurement {
                polarity: MeasurementPolarity::HigherIsBetter,
                calibration: None,
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
                calibration: None,
                observations: Vec::new(),
            }),
        );

        assert!(matches!(result, Err(EdgeError::InvalidEndpoints { .. })));
    }

    #[test]
    fn rejects_measurement_calibration_which_conflicts_with_polarity() {
        let result = Edge::new(
            EntityId::new(1),
            NodeKind::Metric,
            EntityId::new(2),
            NodeKind::Factor,
            EdgePayload::Measures(Measurement {
                polarity: MeasurementPolarity::LowerIsBetter,
                calibration: Some(MeasurementCalibration::Linear {
                    state_zero: 5.0,
                    state_one: 20.0,
                }),
                observations: Vec::new(),
            }),
        );
        assert!(matches!(
            result,
            Err(EdgeError::MeasurementCalibration(
                MeasurementCalibrationError::PolarityMismatch(MeasurementPolarity::LowerIsBetter)
            ))
        ));
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
