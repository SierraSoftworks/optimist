use std::collections::{BTreeMap, BTreeSet};

use super::{Distribution, EntityId, Node, NodePayload, ScenarioAnalysisError};

#[derive(Clone)]
pub(super) struct StateNode {
    pub(super) id: EntityId,
    pub(super) baseline: Distribution,
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
            let baseline = match &node.payload {
                NodePayload::Outcome(outcome) => outcome
                    .current
                    .as_ref()
                    .ok_or(ScenarioAnalysisError::MissingObjectiveBaseline(node.id))?,
                NodePayload::Factor(factor) => factor
                    .current
                    .as_ref()
                    .ok_or(ScenarioAnalysisError::MissingFactorBaseline(node.id))?,
                _ => return Err(ScenarioAnalysisError::MissingCausalNode(node.id)),
            };
            Ok(StateNode {
                id: node.id,
                baseline: baseline.distribution.clone(),
            })
        })
        .collect()
}
