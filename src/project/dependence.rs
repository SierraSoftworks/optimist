use crate::{
    command::{CommandOutcome, RemoveProjectDependence, SetProjectDependence},
    domain::ProjectDependenceModel,
};

use super::{ProjectError, catalog::ProjectEntry, dependence_addresses};

pub(super) fn set(
    entry: &mut ProjectEntry,
    command: SetProjectDependence,
) -> Result<CommandOutcome, ProjectError> {
    command.model.validate_for_project(&entry.project.id)?;
    validate_set_revision(entry, command.model.revision)?;
    dependence_addresses::validate(entry, &command.model)?;
    let revision = match &entry.dependence {
        Some(current) => current.revision.checked_add(1).ok_or_else(|| {
            ProjectError::DependenceRevisionSpaceExhausted(entry.project.id.clone())
        })?,
        None => 0,
    };
    let model = ProjectDependenceModel {
        revision,
        residual_groups: command.model.residual_groups,
    };
    entry.dependence = Some(model.clone());
    Ok(CommandOutcome::ProjectDependenceSet(model))
}

pub(super) fn remove(
    entry: &mut ProjectEntry,
    command: RemoveProjectDependence,
) -> Result<CommandOutcome, ProjectError> {
    validate_existing_revision(entry, command.expected_revision)?;
    let model = entry
        .dependence
        .take()
        .expect("validate_revision found dependence");
    Ok(CommandOutcome::ProjectDependenceRemoved(model))
}

fn validate_set_revision(entry: &ProjectEntry, expected: u64) -> Result<(), ProjectError> {
    match &entry.dependence {
        Some(current) if current.revision != expected => {
            Err(ProjectError::DependenceRevisionConflict {
                expected,
                current: current.revision,
            })
        }
        Some(_) => Ok(()),
        None if expected == 0 => Ok(()),
        None => Err(ProjectError::DependenceNotFound(entry.project.id.clone())),
    }
}

fn validate_existing_revision(entry: &ProjectEntry, expected: u64) -> Result<(), ProjectError> {
    let current = entry
        .dependence
        .as_ref()
        .ok_or_else(|| ProjectError::DependenceNotFound(entry.project.id.clone()))?;
    if current.revision != expected {
        return Err(ProjectError::DependenceRevisionConflict {
            expected,
            current: current.revision,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        command::{
            CommandOutcome, CommandRequest, CreateNode, GraphCommand, RemoveProjectDependence,
            SetNodeQuantityState, SetProjectDependence, SetSquiggleEstimate,
        },
        domain::{
            CorrelationScale, EntityId, EstimateAddress, EstimateId, EstimateOwner, EstimateSlot,
            Factor, GaussianCopulaCorrelation, NodePayload, ProjectDependenceModel, ProjectId,
            QuantityDefinition, QuantitySupport, ResidualDependenceGroup,
            SquiggleEstimateDefinition, Unit,
        },
        project::{ProjectCatalog, ProjectError},
    };

    fn address(project: ProjectId, owner: u64, estimate: u64) -> EstimateAddress {
        EstimateAddress::new(
            project,
            EstimateOwner::Node(EntityId::new(owner)),
            EstimateId::new(estimate),
        )
    }

    fn model(project: &ProjectId, right: u64) -> ProjectDependenceModel {
        ProjectDependenceModel {
            revision: 0,
            residual_groups: vec![ResidualDependenceGroup {
                members: vec![
                    address(project.clone(), 0, 0),
                    address(project.clone(), 1, right),
                ],
                correlation: GaussianCopulaCorrelation {
                    scale: CorrelationScale::Latent,
                    matrix: vec![vec![1.0, 0.5], vec![0.5, 1.0]],
                },
            }],
        }
    }

    fn catalog() -> (ProjectCatalog, ProjectId) {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        let quantity = QuantityDefinition::with_dimension(
            "state",
            Some(Unit::dimensionless()),
            None,
            QuantitySupport::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
        )
        .unwrap();
        for node in 0..2 {
            let revision = node * 3;
            catalog
                .execute(
                    &project.id,
                    CommandRequest::new(
                        revision,
                        GraphCommand::CreateNode(CreateNode {
                            name: format!("factor-{node}"),
                            title: format!("Factor {node}"),
                            payload: NodePayload::Factor(Factor {
                                controllable: false,
                                evidence: vec![],
                            }),
                        }),
                    ),
                )
                .unwrap();
            catalog
                .execute(
                    &project.id,
                    CommandRequest::new(
                        revision + 1,
                        GraphCommand::SetNodeQuantityState(SetNodeQuantityState {
                            node: EntityId::new(node),
                            expected_revision: 0,
                            quantity: quantity.clone(),
                        }),
                    ),
                )
                .unwrap();
            catalog
                .execute(
                    &project.id,
                    CommandRequest::new(
                        revision + 2,
                        GraphCommand::SetSquiggleEstimate(SetSquiggleEstimate {
                            address: address(project.id.clone(), node, 0),
                            slot: EstimateSlot::Current,
                            definition: SquiggleEstimateDefinition {
                                source: "beta(2, 2)".to_owned(),
                                seed: 42,
                                sample_count: 256,
                                target_unit: Unit::dimensionless(),
                            },
                            provenance: vec![],
                            uncertainty: Default::default(),
                        }),
                    ),
                )
                .unwrap();
        }
        (catalog, project.id)
    }

    #[test]
    fn persists_replaces_and_removes_with_idempotent_project_commands() {
        let (mut catalog, project) = catalog();
        let request = CommandRequest::new(
            6,
            GraphCommand::SetProjectDependence(SetProjectDependence {
                model: model(&project, 0),
            }),
        );
        let created = catalog.execute(&project, request.clone()).unwrap();
        assert_eq!(created, catalog.execute(&project, request).unwrap());
        assert_eq!(
            catalog.get_dependence(&project).unwrap().unwrap().revision,
            0
        );

        let mut replacement = model(&project, 0);
        replacement.residual_groups[0].correlation.matrix[0][1] = -0.25;
        replacement.residual_groups[0].correlation.matrix[1][0] = -0.25;
        let updated = catalog
            .execute(
                &project,
                CommandRequest::new(
                    7,
                    GraphCommand::SetProjectDependence(SetProjectDependence { model: replacement }),
                ),
            )
            .unwrap();
        let CommandOutcome::ProjectDependenceSet(updated) = updated.outcome else {
            unreachable!()
        };
        assert_eq!(updated.revision, 1);
        assert!(matches!(
            catalog.execute(
                &project,
                CommandRequest::new(
                    8,
                    GraphCommand::RemoveProjectDependence(RemoveProjectDependence {
                        expected_revision: 0,
                    }),
                ),
            ),
            Err(ProjectError::DependenceRevisionConflict { current: 1, .. })
        ));
        catalog
            .execute(
                &project,
                CommandRequest::new(
                    8,
                    GraphCommand::RemoveProjectDependence(RemoveProjectDependence {
                        expected_revision: 1,
                    }),
                ),
            )
            .unwrap();
        assert_eq!(catalog.get_dependence(&project).unwrap(), None);
    }

    #[test]
    fn rejects_cross_project_and_missing_addresses_without_revision_change() {
        let (mut catalog, project) = catalog();
        let mut cross_project = model(&project, 0);
        cross_project.residual_groups[0].members[1].project = ProjectId::new("foreign").unwrap();
        assert!(matches!(
            catalog.execute(
                &project,
                CommandRequest::new(
                    6,
                    GraphCommand::SetProjectDependence(SetProjectDependence {
                        model: cross_project,
                    }),
                ),
            ),
            Err(ProjectError::Dependence(_))
        ));
        assert!(matches!(
            catalog.execute(
                &project,
                CommandRequest::new(
                    6,
                    GraphCommand::SetProjectDependence(SetProjectDependence {
                        model: model(&project, 9),
                    }),
                ),
            ),
            Err(ProjectError::MissingEstimateAddress(_))
        ));
        assert_eq!(catalog.get(&project).unwrap().revision, 6);
    }
}
