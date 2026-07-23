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
                NodePayload::Outcome(_) | NodePayload::Factor(_) => {
                    let missing = match node.payload {
                        NodePayload::Outcome(_) => {
                            ScenarioAnalysisError::MissingObjectiveBaseline(node.id)
                        }
                        _ => ScenarioAnalysisError::MissingFactorBaseline(node.id),
                    };
                    let state = node.native_state.as_ref().ok_or(missing.clone())?;
                    (
                        state
                            .forecast
                            .as_ref()
                            .or(state.current.as_ref())
                            .ok_or(missing)?
                            .distribution
                            .clone(),
                        quantity_bounds(state.quantity.support),
                    )
                }
                NodePayload::Metric(metric) => (
                    metric
                        .current
                        .as_ref()
                        .ok_or(ScenarioAnalysisError::MissingMetricBaseline(node.id))?
                        .distribution
                        .clone(),
                    quantity_bounds(metric.quantity.support),
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

fn quantity_bounds(support: QuantitySupport) -> StateBounds {
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

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use crate::domain::{
        Distribution, EntityId, Estimate, EstimateId, Factor, Node, NodePayload,
        QuantityDefinition, QuantityState, QuantitySupport, QuantityValue, Unit,
    };

    use super::project;

    #[test]
    fn native_forecast_uses_declared_support_instead_of_zero_one_bounds() {
        let estimate = |id, value| {
            Estimate::<QuantityValue>::new(id, Distribution::point(value).unwrap()).unwrap()
        };
        let mut node = Node::new(
            EntityId::new(0),
            "lead_time",
            "Lead time",
            NodePayload::Factor(Factor {
                controllable: false,
                evidence: vec![],
            }),
        )
        .unwrap();
        node.native_state = Some(
            QuantityState::new(
                QuantityDefinition::with_dimension(
                    "days",
                    Some(Unit::base("day").unwrap()),
                    None,
                    QuantitySupport::NonNegative,
                )
                .unwrap(),
                Some(estimate(EstimateId::new(0), 12.0)),
                Some(estimate(EstimateId::new(1), 15.0)),
            )
            .unwrap(),
        );
        let nodes = BTreeMap::from([(node.id, &node)]);
        let state = project(&nodes, &BTreeSet::from([node.id])).unwrap();

        assert_eq!(state[0].baseline.mean(), 15.0);
        assert_eq!(state[0].bounds.clamp(12.0), 12.0);
        assert_eq!(state[0].bounds.clamp(-1.0), 0.0);
    }
}
