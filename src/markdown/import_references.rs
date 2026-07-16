use std::collections::BTreeMap;

use crate::domain::{Edge, EntityId, NodeKind, ScenarioId};

use super::{
    EntityDocument, ImportError, ProjectDocument, ScenarioDocument, SourceDocument,
    import_dependence,
};

pub(super) fn validate(
    project: &SourceDocument<ProjectDocument>,
    entities: &BTreeMap<EntityId, SourceDocument<EntityDocument>>,
    scenarios: &BTreeMap<ScenarioId, SourceDocument<ScenarioDocument>>,
) -> Result<(), ImportError> {
    for entity in entities.values() {
        for edge in &entity.document.outgoing_edges {
            validate_edge_endpoint(entity, entities, edge, edge.source, edge.source_kind)?;
            validate_edge_endpoint(
                entity,
                entities,
                edge,
                edge.destination,
                edge.destination_kind,
            )?;
        }
    }
    for source in scenarios.values() {
        for objective in &source.document.scenario.draft.objectives {
            validate_scenario_reference(source, entities, objective.outcome_id, NodeKind::Outcome)?;
        }
        for candidate in &source.document.scenario.draft.candidate_interventions {
            validate_scenario_reference(source, entities, *candidate, NodeKind::Intervention)?;
        }
    }
    import_dependence::validate(project, entities)?;
    Ok(())
}

fn validate_edge_endpoint(
    source: &SourceDocument<EntityDocument>,
    entities: &BTreeMap<EntityId, SourceDocument<EntityDocument>>,
    edge: &Edge,
    id: EntityId,
    declared: NodeKind,
) -> Result<(), ImportError> {
    let Some(node) = entities.get(&id).map(|value| &value.document.node) else {
        return Err(ImportError::MissingEdgeEndpoint {
            path: source.path.clone(),
            edge: edge.id(),
            node: id,
        });
    };
    if node.kind() != declared {
        return Err(ImportError::EdgeEndpointKindMismatch {
            path: source.path.clone(),
            edge: edge.id(),
            node: id,
            declared,
            actual: node.kind(),
        });
    }
    Ok(())
}

fn validate_scenario_reference(
    source: &SourceDocument<ScenarioDocument>,
    entities: &BTreeMap<EntityId, SourceDocument<EntityDocument>>,
    id: EntityId,
    expected: NodeKind,
) -> Result<(), ImportError> {
    let actual = entities.get(&id).map(|value| value.document.node.kind());
    if actual != Some(expected) {
        return Err(ImportError::InvalidScenarioReference {
            path: source.path.clone(),
            scenario: source.document.scenario.id,
            node: id,
            expected,
            actual,
        });
    }
    Ok(())
}
