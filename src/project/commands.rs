use crate::{
    command::{ChangeSet, CommandRequest, CommandResult},
    domain::ProjectId,
};

use super::{ProjectCatalog, ProjectError, apply};

impl ProjectCatalog {
    /// Applies a typed graph command under project revision and idempotency checks.
    ///
    /// A duplicate request ID returns its original result before comparing revisions.
    /// New commands must match the current revision and are serialized by the mutable
    /// catalog borrow used by the server's per-project write path.
    pub fn execute(
        &mut self,
        project_id: &ProjectId,
        request: CommandRequest,
    ) -> Result<CommandResult, ProjectError> {
        let entry = self
            .projects
            .get_mut(project_id)
            .ok_or_else(|| ProjectError::NotFound(project_id.clone()))?;
        if let Some(result) = entry.results.get(&request.request_id) {
            return Ok(result.clone());
        }
        if request.expected_revision != entry.project.revision {
            return Err(ProjectError::RevisionConflict {
                expected: request.expected_revision,
                current: entry.project.revision,
            });
        }

        let next_revision = entry
            .project
            .revision
            .checked_add(1)
            .ok_or_else(|| ProjectError::RevisionSpaceExhausted(project_id.clone()))?;
        let changes_graph = request.command.changes_graph();
        let command = request.command.clone();
        let next_graph_revision = if changes_graph {
            entry
                .graph_revision
                .checked_add(1)
                .ok_or_else(|| ProjectError::GraphRevisionSpaceExhausted(project_id.clone()))?
        } else {
            entry.graph_revision
        };
        let outcome = apply::command(entry, request.command)?;
        entry.project.revision = next_revision;
        entry.graph_revision = next_graph_revision;
        let result = CommandResult {
            request_id: request.request_id,
            project_revision: entry.project.revision,
            outcome,
        };
        entry.changes.insert(
            entry.project.revision,
            ChangeSet {
                request_id: request.request_id,
                base_revision: request.expected_revision,
                project_revision: entry.project.revision,
                graph_revision: entry.graph_revision,
                command,
                outcome: result.outcome.clone(),
            },
        );
        entry.results.insert(request.request_id, result.clone());
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        command::{
            AppendObservation, CommandOutcome, CommandRequest, CorrectObservation, CreateEdge,
            CreateNode, DeleteEdge, DeleteNode, GraphCommand, SetMeasurementCalibration,
        },
        domain::{
            EdgeId, EdgeKind, EdgePayload, EntityId, Factor, Measurement, MeasurementCalibration,
            MeasurementCalibrationError, MeasurementPolarity, Metric, NewObservation, NodePayload,
            Requirement,
        },
    };

    use super::ProjectCatalog;
    use crate::project::{AggregateUpdateError, ProjectError};

    fn create_node(revision: u64) -> CommandRequest {
        CommandRequest::new(
            revision,
            GraphCommand::CreateNode(CreateNode {
                name: "github".to_owned(),
                title: "GitHub".to_owned(),
                payload: NodePayload::Factor(Factor {
                    current: None,
                    desired: None,
                    controllable: false,
                    evidence: vec![],
                }),
            }),
        )
    }

    #[test]
    fn calibrates_measurement_values_under_edge_revision_guards() {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    0,
                    GraphCommand::CreateNode(CreateNode {
                        name: "lead_time".to_owned(),
                        title: "Lead time".to_owned(),
                        payload: NodePayload::Metric(Metric {
                            unit: "days".to_owned(),
                            aggregation: None,
                        }),
                    }),
                ),
            )
            .unwrap();
        catalog.execute(&project.id, create_node(1)).unwrap();
        let edge_id = EdgeId {
            source: EntityId::new(0),
            kind: EdgeKind::Measures,
            destination: EntityId::new(1),
        };
        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    2,
                    GraphCommand::CreateEdge(CreateEdge {
                        source: edge_id.source,
                        destination: edge_id.destination,
                        payload: EdgePayload::Measures(Measurement {
                            polarity: MeasurementPolarity::LowerIsBetter,
                            calibration: None,
                            observations: vec![],
                        }),
                    }),
                ),
            )
            .unwrap();
        let result = catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    3,
                    GraphCommand::SetMeasurementCalibration(SetMeasurementCalibration {
                        edge: edge_id.clone(),
                        expected_revision: 0,
                        calibration: Some(MeasurementCalibration::Linear {
                            state_zero: 20.0,
                            state_one: 5.0,
                        }),
                    }),
                ),
            )
            .unwrap();
        let CommandOutcome::MeasurementCalibrationSet(edge) = result.outcome else {
            panic!("expected calibrated edge")
        };
        assert_eq!(edge.revision, 1);
        let EdgePayload::Measures(measurement) = edge.payload else {
            panic!("expected measurement payload")
        };
        assert_eq!(measurement.calibration.unwrap().state(12.5).unwrap(), 0.5);

        let mismatched = catalog.execute(
            &project.id,
            CommandRequest::new(
                4,
                GraphCommand::SetMeasurementCalibration(SetMeasurementCalibration {
                    edge: edge_id.clone(),
                    expected_revision: 1,
                    calibration: Some(MeasurementCalibration::Linear {
                        state_zero: 5.0,
                        state_one: 20.0,
                    }),
                }),
            ),
        );
        assert!(matches!(
            mismatched,
            Err(ProjectError::MeasurementCalibration(
                MeasurementCalibrationError::PolarityMismatch(MeasurementPolarity::LowerIsBetter)
            ))
        ));
        assert!(matches!(
            catalog.execute(
                &project.id,
                CommandRequest::new(
                    4,
                    GraphCommand::SetMeasurementCalibration(SetMeasurementCalibration {
                        edge: edge_id,
                        expected_revision: 0,
                        calibration: None,
                    }),
                ),
            ),
            Err(ProjectError::AggregateUpdate(
                AggregateUpdateError::EdgeRevisionConflict { current: 1, .. }
            ))
        ));
    }

    #[test]
    fn applies_commands_idempotently_and_advances_revision() {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        let request = create_node(0);
        let first = catalog.execute(&project.id, request.clone()).unwrap();
        let retry = catalog.execute(&project.id, request).unwrap();

        assert_eq!(first, retry);
        assert_eq!(first.project_revision, 1);
        assert!(matches!(first.outcome, CommandOutcome::NodeCreated(_)));
        assert_eq!(catalog.list_nodes(&project.id).unwrap().len(), 1);
        let replay = catalog.replay_changes(&project.id, 0).unwrap();
        assert_eq!(replay.current_revision, 1);
        assert_eq!(replay.changes.len(), 1);
        assert_eq!(replay.changes[0].request_id, first.request_id);
        assert_eq!(replay.changes[0].base_revision, 0);
        assert_eq!(replay.changes[0].project_revision, 1);
        assert_eq!(replay.changes[0].graph_revision, 1);
    }

    #[test]
    fn replays_changes_in_revision_order_after_exclusive_cursor() {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        catalog.execute(&project.id, create_node(0)).unwrap();
        let mut second = create_node(1);
        let GraphCommand::CreateNode(node) = &mut second.command else {
            unreachable!()
        };
        node.name = "quality".to_owned();
        node.title = "Quality".to_owned();
        catalog.execute(&project.id, second).unwrap();
        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    2,
                    GraphCommand::CreateEdge(CreateEdge {
                        source: EntityId::new(0),
                        destination: EntityId::new(1),
                        payload: EdgePayload::Requires(Requirement {
                            hard: true,
                            satisfaction_threshold: None,
                        }),
                    }),
                ),
            )
            .unwrap();

        let replay = catalog.replay_changes(&project.id, 1).unwrap();
        assert_eq!(
            replay
                .changes
                .iter()
                .map(|change| change.project_revision)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert!(
            catalog
                .replay_changes(&project.id, 3)
                .unwrap()
                .changes
                .is_empty()
        );
        assert!(matches!(
            catalog.replay_changes(&project.id, 4),
            Err(ProjectError::InvalidReplayRevision {
                requested: 4,
                current: 3
            })
        ));
    }

    #[test]
    fn rejects_stale_commands_without_mutating_graph() {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        catalog.execute(&project.id, create_node(0)).unwrap();
        let error = catalog.execute(&project.id, create_node(0)).unwrap_err();

        assert!(matches!(
            error,
            ProjectError::RevisionConflict { current: 1, .. }
        ));
        assert_eq!(catalog.list_nodes(&project.id).unwrap().len(), 1);
    }

    #[test]
    fn creates_edges_idempotently_from_stored_endpoint_kinds() {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        catalog.execute(&project.id, create_node(0)).unwrap();
        let mut second = create_node(1);
        let GraphCommand::CreateNode(node) = &mut second.command else {
            unreachable!()
        };
        node.name = "actions".to_owned();
        node.title = "Actions".to_owned();
        catalog.execute(&project.id, second).unwrap();

        let request = CommandRequest::new(
            2,
            GraphCommand::CreateEdge(CreateEdge {
                source: crate::domain::EntityId::new(1),
                destination: crate::domain::EntityId::new(0),
                payload: EdgePayload::Requires(Requirement {
                    hard: true,
                    satisfaction_threshold: None,
                }),
            }),
        );
        let first = catalog.execute(&project.id, request.clone()).unwrap();
        assert_eq!(first, catalog.execute(&project.id, request).unwrap());
        assert!(matches!(first.outcome, CommandOutcome::EdgeCreated(_)));
        assert_eq!(catalog.list_edges(&project.id).unwrap().len(), 1);
    }

    #[test]
    fn deletes_edges_before_nodes_with_revision_and_retry_guards() {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        catalog.execute(&project.id, create_node(0)).unwrap();
        let mut second = create_node(1);
        let GraphCommand::CreateNode(node) = &mut second.command else {
            unreachable!()
        };
        node.name = "actions".to_owned();
        catalog.execute(&project.id, second).unwrap();
        let edge_id = EdgeId {
            source: crate::domain::EntityId::new(1),
            kind: EdgeKind::Requires,
            destination: crate::domain::EntityId::new(0),
        };
        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    2,
                    GraphCommand::CreateEdge(CreateEdge {
                        source: edge_id.source,
                        destination: edge_id.destination,
                        payload: EdgePayload::Requires(Requirement {
                            hard: true,
                            satisfaction_threshold: None,
                        }),
                    }),
                ),
            )
            .unwrap();

        let blocked = catalog.execute(
            &project.id,
            CommandRequest::new(
                3,
                GraphCommand::DeleteNode(DeleteNode {
                    id: edge_id.destination,
                }),
            ),
        );
        assert!(matches!(
            blocked,
            Err(ProjectError::Repository(
                crate::store::RepositoryError::EntityHasEdges(_)
            ))
        ));
        assert_eq!(catalog.get(&project.id).unwrap().revision, 3);

        let stale = catalog.execute(
            &project.id,
            CommandRequest::new(
                2,
                GraphCommand::DeleteEdge(DeleteEdge {
                    id: edge_id.clone(),
                }),
            ),
        );
        assert!(matches!(stale, Err(ProjectError::RevisionConflict { .. })));

        let delete_edge = CommandRequest::new(
            3,
            GraphCommand::DeleteEdge(DeleteEdge {
                id: edge_id.clone(),
            }),
        );
        let deleted = catalog.execute(&project.id, delete_edge.clone()).unwrap();
        assert_eq!(deleted, catalog.execute(&project.id, delete_edge).unwrap());
        assert!(matches!(deleted.outcome, CommandOutcome::EdgeDeleted(_)));

        let deleted_node = catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    4,
                    GraphCommand::DeleteNode(DeleteNode {
                        id: edge_id.destination,
                    }),
                ),
            )
            .unwrap();
        assert!(matches!(
            deleted_node.outcome,
            CommandOutcome::NodeDeleted(_)
        ));
        assert!(
            catalog
                .get_node(&project.id, edge_id.destination)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn appends_and_corrects_measurement_observations() {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        let metric = CommandRequest::new(
            0,
            GraphCommand::CreateNode(CreateNode {
                name: "availability".to_owned(),
                title: "Availability".to_owned(),
                payload: NodePayload::Metric(Metric {
                    unit: "ratio".to_owned(),
                    aggregation: None,
                }),
            }),
        );
        catalog.execute(&project.id, metric).unwrap();
        let factor = create_node(1);
        catalog.execute(&project.id, factor).unwrap();
        let edge_id = EdgeId {
            source: crate::domain::EntityId::new(0),
            kind: EdgeKind::Measures,
            destination: crate::domain::EntityId::new(1),
        };
        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    2,
                    GraphCommand::CreateEdge(CreateEdge {
                        source: edge_id.source,
                        destination: edge_id.destination,
                        payload: EdgePayload::Measures(Measurement {
                            polarity: MeasurementPolarity::HigherIsBetter,
                            calibration: None,
                            observations: vec![],
                        }),
                    }),
                ),
            )
            .unwrap();
        let append = CommandRequest::new(
            3,
            GraphCommand::AppendObservation(AppendObservation {
                edge: edge_id.clone(),
                observation: NewObservation {
                    value: 0.9,
                    unit: "ratio".to_owned(),
                    observed_at: "2026-07-15T12:00:00Z".to_owned(),
                    source: "dashboard".to_owned(),
                    measurement_standard_deviation: Some(0.02),
                },
            }),
        );
        let appended = catalog.execute(&project.id, append.clone()).unwrap();
        assert_eq!(appended, catalog.execute(&project.id, append).unwrap());
        let corrected = catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    4,
                    GraphCommand::CorrectObservation(CorrectObservation {
                        edge: edge_id.clone(),
                        observation_id: 0,
                        value: 0.95,
                    }),
                ),
            )
            .unwrap();
        assert!(matches!(
            corrected.outcome,
            CommandOutcome::ObservationCorrected { .. }
        ));
        let edge = catalog.get_edge(&project.id, &edge_id).unwrap().unwrap();
        let EdgePayload::Measures(measurement) = edge.payload else {
            panic!("expected measurement edge")
        };
        assert_eq!(measurement.observations.len(), 2);
        assert_eq!(measurement.observations[1].supersedes, Some(0));

        let invalid = CommandRequest::new(
            5,
            GraphCommand::AppendObservation(AppendObservation {
                edge: edge_id,
                observation: NewObservation {
                    value: 90.0,
                    unit: "percent".to_owned(),
                    observed_at: "2026-07-15T13:00:00Z".to_owned(),
                    source: "dashboard".to_owned(),
                    measurement_standard_deviation: None,
                },
            }),
        );
        assert!(matches!(
            catalog.execute(&project.id, invalid),
            Err(ProjectError::ObservationUnitMismatch { .. })
        ));
        assert_eq!(catalog.get(&project.id).unwrap().revision, 5);
    }
}
