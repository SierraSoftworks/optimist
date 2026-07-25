use std::collections::{BTreeMap, BTreeSet};

use super::{
    EntityId, EstimateOwner, Node, NodePayload, QuantitySupport, ScenarioAnalysisError,
    scenario_analysis_coupling::{CoupledPrimitive, Coupling},
};

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

/// How a state combines the proportional responses reaching it.
///
/// The rule follows the quantity's declared support rather than being authored,
/// because support is what makes one rule sound. A strictly non-negative quantity
/// such as a rate or a duration composes multiplicatively, which keeps it
/// non-negative for free and makes a plain product expressible with unit
/// elasticities. A quantity that may be zero or negative has no meaningful ratio
/// scale, so its responses accumulate against its baseline instead.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Combination {
    /// Responses multiply: $x_i = b_i \prod_j (x_j/b_j)^{\varepsilon_{ji}}$.
    Multiplicative,
    /// Responses accumulate: $x_i = b_i (1 + \sum_j \varepsilon_{ji}(x_j/b_j - 1))$.
    Additive,
}

#[derive(Clone)]
pub(super) struct StateNode {
    pub(super) id: EntityId,
    pub(super) baseline: CoupledPrimitive,
    pub(super) bounds: StateBounds,
    pub(super) combination: Combination,
}

pub(super) fn project(
    nodes: &BTreeMap<EntityId, &Node>,
    relevant: &BTreeSet<EntityId>,
    coupling: &Coupling,
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
                            .ok_or(missing)?,
                        quantity_bounds(state.quantity.support),
                    )
                }
                NodePayload::Metric(metric) => (
                    metric
                        .current
                        .as_ref()
                        .ok_or(ScenarioAnalysisError::MissingMetricBaseline(node.id))?,
                    quantity_bounds(metric.quantity.support),
                ),
                _ => return Err(ScenarioAnalysisError::MissingCausalNode(node.id)),
            };
            Ok(StateNode {
                id: node.id,
                baseline: coupling.primitive(
                    &EstimateOwner::Node(node.id),
                    baseline.id,
                    &baseline.distribution,
                ),
                bounds,
                combination: combination(support(node)),
            })
        })
        .collect()
}

fn support(node: &Node) -> QuantitySupport {
    match &node.payload {
        NodePayload::Metric(metric) => metric.quantity.support,
        _ => node
            .native_state
            .as_ref()
            .map_or(QuantitySupport::Real, |state| state.quantity.support),
    }
}

fn combination(support: QuantitySupport) -> Combination {
    match support {
        QuantitySupport::NonNegative => Combination::Multiplicative,
        QuantitySupport::Real | QuantitySupport::Bounded { .. } => Combination::Additive,
    }
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
        let state = project(
            &nodes,
            &BTreeSet::from([node.id]),
            &super::super::scenario_analysis_coupling::Coupling::default(),
        )
        .unwrap();

        assert_eq!(state[0].baseline.marginal_mean(), 15.0);
        assert_eq!(state[0].bounds.clamp(12.0), 12.0);
        assert_eq!(state[0].bounds.clamp(-1.0), 0.0);
    }
}
