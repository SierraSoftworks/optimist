use crate::{
    command::{CommandOutcome, GraphCommand},
    domain::{Edge, EdgePayload, Node, NodePayload},
    store::{GraphRepository, RepositoryError},
};

use super::{
    ProjectError, aggregate_updates, catalog::ProjectEntry, dependence, estimate, evidence,
    node_state, scenarios,
};

pub(super) fn command(
    entry: &mut ProjectEntry,
    command: GraphCommand,
) -> Result<CommandOutcome, ProjectError> {
    match command {
        GraphCommand::CreateNode(command) => {
            let id = entry.repository.next_entity_id()?;
            let node = Node::new(id, command.name, command.title, command.payload)?;
            entry.repository.create_node(node.clone())?;
            Ok(CommandOutcome::NodeCreated(node))
        }
        GraphCommand::DeleteNode(command) => {
            let node = entry.repository.delete_node(command.id)?;
            Ok(CommandOutcome::NodeDeleted(node))
        }
        GraphCommand::UpdateNodeMetadata(command) => aggregate_updates::node(entry, command),
        GraphCommand::SetNodeQuantityState(command) => node_state::set(entry, command),
        GraphCommand::CreateEvidence(command) => evidence::create(entry, command),
        GraphCommand::UpdateEvidence(command) => evidence::update(entry, command),
        GraphCommand::DeleteEvidence(command) => evidence::delete(entry, command),
        GraphCommand::CreateEdge(command) => {
            let source = entry
                .repository
                .get_node(command.source)?
                .ok_or(RepositoryError::MissingEntity(command.source))?;
            let destination = entry
                .repository
                .get_node(command.destination)?
                .ok_or(RepositoryError::MissingEntity(command.destination))?;
            let edge = Edge::new(
                command.source,
                source.kind(),
                command.destination,
                destination.kind(),
                command.payload,
            )
            .map_err(RepositoryError::from)?;
            validate_causal_units(&source, &destination, &edge)?;
            entry.repository.create_edge(edge.clone())?;
            Ok(CommandOutcome::EdgeCreated(edge))
        }
        GraphCommand::DeleteEdge(command) => {
            let edge = entry.repository.delete_edge(&command.id)?;
            Ok(CommandOutcome::EdgeDeleted(edge))
        }
        GraphCommand::UpdateEdgeMetadata(command) => aggregate_updates::edge(entry, command),
        GraphCommand::AppendObservation(command) => {
            let mut edge = measurement_edge(entry, &command.edge)?;
            validate_metric_unit(entry, &edge, &command.observation.unit)?;
            let next_revision = next_edge_revision(&edge)?;
            let EdgePayload::Measures(measurement) = &mut edge.payload else {
                unreachable!("measurement_edge validated the payload")
            };
            let observation = measurement.append(command.observation)?;
            edge.revision = next_revision;
            entry.repository.update_edge(edge.clone())?;
            Ok(CommandOutcome::ObservationAppended { edge, observation })
        }
        GraphCommand::CorrectObservation(command) => {
            let mut edge = measurement_edge(entry, &command.edge)?;
            let next_revision = next_edge_revision(&edge)?;
            let EdgePayload::Measures(measurement) = &mut edge.payload else {
                unreachable!("measurement_edge validated the payload")
            };
            let observation = measurement.correct(command.observation_id, command.value)?;
            edge.revision = next_revision;
            entry.repository.update_edge(edge.clone())?;
            Ok(CommandOutcome::ObservationCorrected { edge, observation })
        }
        GraphCommand::SetMeasurementCalibration(command) => {
            let mut edge = measurement_edge(entry, &command.edge)?;
            if edge.revision != command.expected_revision {
                return Err(super::AggregateUpdateError::EdgeRevisionConflict {
                    id: command.edge,
                    expected: command.expected_revision,
                    current: edge.revision,
                }
                .into());
            }
            let next_revision = next_edge_revision(&edge)?;
            let EdgePayload::Measures(measurement) = &mut edge.payload else {
                unreachable!("measurement_edge validated the payload")
            };
            measurement.set_calibration(command.calibration)?;
            edge.revision = next_revision;
            entry.repository.update_edge(edge.clone())?;
            Ok(CommandOutcome::MeasurementCalibrationSet(edge))
        }
        GraphCommand::SetSquiggleEstimate(command) => estimate::set_squiggle(entry, command),
        GraphCommand::RemoveEstimate(command) => estimate::remove(entry, command),
        GraphCommand::CreateScenario(command) => scenarios::create(entry, command),
        GraphCommand::UpdateScenario(command) => scenarios::update(entry, command),
        GraphCommand::DeleteScenario(command) => scenarios::delete(entry, command),
        GraphCommand::SetProjectDependence(command) => dependence::set(entry, command),
        GraphCommand::RemoveProjectDependence(command) => dependence::remove(entry, command),
    }
}

fn validate_causal_units(
    source: &Node,
    destination: &Node,
    edge: &Edge,
) -> Result<(), ProjectError> {
    let effect = match &edge.payload {
        EdgePayload::Contributes(effect) | EdgePayload::Changes(effect) => effect,
        _ => return Ok(()),
    };
    let response = &effect.response;
    let source_unit = if matches!(edge.payload, EdgePayload::Changes(_)) {
        crate::domain::Unit::dimensionless()
    } else {
        state_unit(source)?
    };
    let destination_unit = state_unit(destination)?;
    if response.source_unit != source_unit || response.destination_unit != destination_unit {
        return Err(ProjectError::CausalResponseUnitMismatch {
            edge: edge.id(),
            expected_source: source_unit,
            actual_source: response.source_unit.clone(),
            expected_destination: destination_unit,
            actual_destination: response.destination_unit.clone(),
        });
    }
    Ok(())
}

fn state_unit(node: &Node) -> Result<crate::domain::Unit, ProjectError> {
    if let Some(state) = &node.native_state {
        return state
            .quantity
            .dimension
            .clone()
            .ok_or(ProjectError::MissingQuantityDimension(node.id));
    }
    match &node.payload {
        NodePayload::Metric(metric) => metric
            .quantity
            .dimension
            .clone()
            .ok_or(ProjectError::MissingQuantityDimension(node.id)),
        NodePayload::Factor(_) | NodePayload::Outcome(_) => {
            Err(ProjectError::MissingQuantityDimension(node.id))
        }
        NodePayload::Intervention(_) => Err(ProjectError::MissingQuantityDimension(node.id)),
    }
}

fn measurement_edge(
    entry: &mut ProjectEntry,
    id: &crate::domain::EdgeId,
) -> Result<Edge, ProjectError> {
    let edge = entry
        .repository
        .get_edge(id)?
        .ok_or_else(|| RepositoryError::MissingEdge(id.to_string()))?;
    if !matches!(edge.payload, EdgePayload::Measures(_)) {
        return Err(ProjectError::NotMeasurementEdge(id.clone()));
    }
    Ok(edge)
}

fn validate_metric_unit(
    entry: &mut ProjectEntry,
    edge: &Edge,
    actual: &str,
) -> Result<(), ProjectError> {
    let source = entry
        .repository
        .get_node(edge.source)?
        .ok_or(RepositoryError::MissingEntity(edge.source))?;
    let NodePayload::Metric(metric) = source.payload else {
        return Err(ProjectError::NotMeasurementEdge(edge.id()));
    };
    if metric.quantity.unit != actual {
        return Err(ProjectError::ObservationUnitMismatch {
            expected: metric.quantity.unit,
            actual: actual.to_owned(),
        });
    }
    Ok(())
}

fn next_edge_revision(edge: &Edge) -> Result<u64, ProjectError> {
    edge.revision
        .checked_add(1)
        .ok_or_else(|| ProjectError::EdgeRevisionSpaceExhausted(edge.id()))
}
