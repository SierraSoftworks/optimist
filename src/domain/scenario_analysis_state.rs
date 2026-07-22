use std::collections::{BTreeMap, BTreeSet};

use super::{Distribution, EntityId, Node, NodePayload, QuantitySupport, ScenarioAnalysisError};

#[derive(Clone, Copy)]
pub(super) struct StateBounds {
    lower: Option<f64>,
    upper: Option<f64>,
}

impl StateBounds {
    pub(super) fn clamp(self, value: f64) -> f64 {
        let value = self.lower.map_or(value, |lower| value.max(lower));
        self.upper.map_or(value, |upper| value.min(upper))
    }
}

#[derive(Clone)]
pub(super) struct StateNode {
    pub(super) id: EntityId,
    pub(super) baseline: Distribution,
    pub(super) bounds: StateBounds,
}

pub(super) fn project(
    nodes: &BTreeMap<EntityId, &Node>,
    relevant: &BTreeSet<EntityId>,
) -> Result<Vec<StateNode>, ScenarioAnalysisError> {
    relevant
        .iter()
        .map(|id| {
            let node = nodes
                .get(id)
                .ok_or(ScenarioAnalysisError::MissingCausalNode(*id))?;
            let (baseline, bounds) = match &node.payload {
                NodePayload::Outcome(outcome) => (
                    outcome
                        .current
                        .as_ref()
                        .ok_or(ScenarioAnalysisError::MissingObjectiveBaseline(node.id))?
                        .distribution
                        .clone(),
                    StateBounds {
                        lower: Some(0.0),
                        upper: Some(1.0),
                    },
                ),
                NodePayload::Factor(factor) => (
                    factor
                        .current
                        .as_ref()
                        .ok_or(ScenarioAnalysisError::MissingFactorBaseline(node.id))?
                        .distribution
                        .clone(),
                    StateBounds {
                        lower: Some(0.0),
                        upper: Some(1.0),
                    },
                ),
                NodePayload::Metric(metric) => (
                    metric
                        .current
                        .as_ref()
                        .ok_or(ScenarioAnalysisError::MissingMetricBaseline(node.id))?
                        .distribution
                        .clone(),
                    metric_bounds(metric.quantity.support),
                ),
                _ => return Err(ScenarioAnalysisError::MissingCausalNode(node.id)),
            };
            Ok(StateNode {
                id: node.id,
                baseline,
                bounds,
            })
        })
        .collect()
}

fn metric_bounds(support: QuantitySupport) -> StateBounds {
    match support {
        QuantitySupport::Real => StateBounds {
            lower: None,
            upper: None,
        },
        QuantitySupport::NonNegative => StateBounds {
            lower: Some(0.0),
            upper: None,
        },
        QuantitySupport::Bounded { lower, upper } => StateBounds {
            lower: Some(lower),
            upper: Some(upper),
        },
    }
}
