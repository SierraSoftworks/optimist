use crate::{
    command::{CommandOutcome, GraphCommand},
    domain::{Edge, EdgePayload, Node, NodePayload},
    store::{GraphRepository, RepositoryError},
};

use super::{ProjectError, catalog::ProjectEntry, dependence, estimate, scenarios};

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
            entry.repository.create_edge(edge.clone())?;
            Ok(CommandOutcome::EdgeCreated(edge))
        }
        GraphCommand::DeleteEdge(command) => {
            let edge = entry.repository.delete_edge(&command.id)?;
            Ok(CommandOutcome::EdgeDeleted(edge))
        }
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
        GraphCommand::SetEstimate(command) => estimate::set(entry, command),
        GraphCommand::RemoveEstimate(command) => estimate::remove(entry, command),
        GraphCommand::CreateScenario(command) => scenarios::create(entry, command),
        GraphCommand::UpdateScenario(command) => scenarios::update(entry, command),
        GraphCommand::DeleteScenario(command) => scenarios::delete(entry, command),
        GraphCommand::SetProjectDependence(command) => dependence::set(entry, command),
        GraphCommand::RemoveProjectDependence(command) => dependence::remove(entry, command),
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
    if metric.unit != actual {
        return Err(ProjectError::ObservationUnitMismatch {
            expected: metric.unit,
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
