use crate::{
    command::{CommandRequest, CommandResult},
    domain::{
        Edge, EdgeId, EntityId, EstimateAddress, Node, PrimitiveEstimate, ProjectDependenceModel,
        ProjectId, Scenario, ScenarioId,
    },
    store::GraphRepository,
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
        let outcome = apply::command(entry, request.command)?;
        entry.project.revision = next_revision;
        let result = CommandResult {
            request_id: request.request_id,
            project_revision: entry.project.revision,
            outcome,
        };
        entry.results.insert(request.request_id, result.clone());
        Ok(result)
    }

    /// Lists complete node aggregates for one project in deterministic ID order.
    pub fn list_nodes(&mut self, project_id: &ProjectId) -> Result<Vec<Node>, ProjectError> {
        Ok(self.repository_mut(project_id)?.list_nodes()?)
    }

    /// Retrieves one complete node aggregate from a project-local entity ID.
    pub fn get_node(
        &mut self,
        project_id: &ProjectId,
        entity_id: EntityId,
    ) -> Result<Option<Node>, ProjectError> {
        Ok(self.repository_mut(project_id)?.get_node(entity_id)?)
    }

    /// Lists complete edge aggregates for one project in canonical edge-ID order.
    pub fn list_edges(&mut self, project_id: &ProjectId) -> Result<Vec<Edge>, ProjectError> {
        Ok(self.repository_mut(project_id)?.list_edges()?)
    }

    /// Retrieves one complete edge aggregate from its project-local tuple identity.
    pub fn get_edge(
        &mut self,
        project_id: &ProjectId,
        edge_id: &EdgeId,
    ) -> Result<Option<Edge>, ProjectError> {
        Ok(self.repository_mut(project_id)?.get_edge(edge_id)?)
    }

    /// Retrieves one primitive estimate by its stable project/owner-local address.
    pub fn get_estimate(
        &mut self,
        project_id: &ProjectId,
        address: &EstimateAddress,
    ) -> Result<PrimitiveEstimate, ProjectError> {
        let entry = self
            .projects
            .get_mut(project_id)
            .ok_or_else(|| ProjectError::NotFound(project_id.clone()))?;
        super::estimate::get(entry, address)
    }

    /// Lists scenario documents in deterministic project-local ID order.
    pub fn list_scenarios(&self, project_id: &ProjectId) -> Result<Vec<Scenario>, ProjectError> {
        let entry = self
            .projects
            .get(project_id)
            .ok_or_else(|| ProjectError::NotFound(project_id.clone()))?;
        Ok(entry.scenarios.values().cloned().collect())
    }

    /// Retrieves one scenario document without exposing graph storage internals.
    pub fn get_scenario(
        &self,
        project_id: &ProjectId,
        scenario_id: ScenarioId,
    ) -> Result<Option<Scenario>, ProjectError> {
        let entry = self
            .projects
            .get(project_id)
            .ok_or_else(|| ProjectError::NotFound(project_id.clone()))?;
        Ok(entry.scenarios.get(&scenario_id).cloned())
    }

    /// Retrieves the singleton project dependence document outside graph storage.
    pub fn get_dependence(
        &self,
        project_id: &ProjectId,
    ) -> Result<Option<ProjectDependenceModel>, ProjectError> {
        let entry = self
            .projects
            .get(project_id)
            .ok_or_else(|| ProjectError::NotFound(project_id.clone()))?;
        Ok(entry.dependence.clone())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        command::{
            AppendObservation, CommandOutcome, CommandRequest, CorrectObservation, CreateEdge,
            CreateNode, DeleteEdge, DeleteNode, GraphCommand,
        },
        domain::{
            EdgeId, EdgeKind, EdgePayload, Factor, Measurement, MeasurementPolarity, Metric,
            NewObservation, NodePayload, Requirement,
        },
    };

    use super::ProjectCatalog;
    use crate::project::ProjectError;

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
