use crate::{
    command::{CommandOutcome, RemoveEstimate, SetEstimate},
    domain::{EstimateAddress, EstimateOwner, PrimitiveEstimate, ProjectId},
    store::{GraphRepository, RepositoryError},
};

use super::{
    EstimateCommandError, ProjectError, catalog::ProjectEntry, estimate_edge, estimate_node,
    estimate_node_find, estimate_node_remove,
};

pub(super) fn set(
    entry: &mut ProjectEntry,
    command: SetEstimate,
) -> Result<CommandOutcome, ProjectError> {
    validate_address(&entry.project.id, &command.address)?;
    let slot = command
        .slot
        .validated()
        .map_err(EstimateCommandError::from)?;
    let value = match &command.address.owner {
        EstimateOwner::Node(id) => {
            let mut node = entry
                .repository
                .get_node(*id)?
                .ok_or(RepositoryError::MissingEntity(*id))?;
            let value = estimate_node::set(
                &mut node,
                &command.address,
                slot,
                command.distribution,
                command.provenance,
            )?;
            node.revision = next_owner_revision(node.revision, &command.address)?;
            entry.repository.update_node(node)?;
            value
        }
        EstimateOwner::Edge(id) => {
            let mut edge = entry
                .repository
                .get_edge(id)?
                .ok_or_else(|| RepositoryError::MissingEdge(id.to_string()))?;
            let value = estimate_edge::set(
                &mut edge,
                &command.address,
                slot,
                command.distribution,
                command.provenance,
            )?;
            edge.revision = next_owner_revision(edge.revision, &command.address)?;
            entry.repository.update_edge(edge)?;
            value
        }
    };
    Ok(CommandOutcome::EstimateSet(value))
}

pub(super) fn remove(
    entry: &mut ProjectEntry,
    command: RemoveEstimate,
) -> Result<CommandOutcome, ProjectError> {
    validate_address(&entry.project.id, &command.address)?;
    if entry.dependence.as_ref().is_some_and(|model| {
        model
            .residual_groups
            .iter()
            .flat_map(|group| &group.members)
            .any(|member| member == &command.address)
    }) {
        return Err(EstimateCommandError::ReferencedByDependence(command.address).into());
    }
    let value = match &command.address.owner {
        EstimateOwner::Node(id) => {
            let mut node = entry
                .repository
                .get_node(*id)?
                .ok_or(RepositoryError::MissingEntity(*id))?;
            let value = estimate_node_remove::remove(&mut node, &command.address)?;
            node.revision = next_owner_revision(node.revision, &command.address)?;
            entry.repository.update_node(node)?;
            value
        }
        EstimateOwner::Edge(id) => {
            let mut edge = entry
                .repository
                .get_edge(id)?
                .ok_or_else(|| RepositoryError::MissingEdge(id.to_string()))?;
            let value = estimate_edge::remove(&mut edge, &command.address)?;
            edge.revision = next_owner_revision(edge.revision, &command.address)?;
            entry.repository.update_edge(edge)?;
            value
        }
    };
    Ok(CommandOutcome::EstimateRemoved(value))
}

pub(super) fn get(
    entry: &mut ProjectEntry,
    address: &EstimateAddress,
) -> Result<PrimitiveEstimate, ProjectError> {
    validate_address(&entry.project.id, address)?;
    match &address.owner {
        EstimateOwner::Node(id) => {
            let node = entry
                .repository
                .get_node(*id)?
                .ok_or(RepositoryError::MissingEntity(*id))?;
            estimate_node_find::find(&node, address)
        }
        EstimateOwner::Edge(id) => {
            let edge = entry
                .repository
                .get_edge(id)?
                .ok_or_else(|| RepositoryError::MissingEdge(id.to_string()))?;
            estimate_edge::find(&edge, address)
        }
    }
}

fn validate_address(project: &ProjectId, address: &EstimateAddress) -> Result<(), ProjectError> {
    if &address.project != project {
        return Err(EstimateCommandError::CrossProjectAddress(address.clone()).into());
    }
    if !address.components.is_empty() {
        return Err(EstimateCommandError::NestedAddress(address.clone()).into());
    }
    Ok(())
}

fn next_owner_revision(current: u64, address: &EstimateAddress) -> Result<u64, ProjectError> {
    current
        .checked_add(1)
        .ok_or_else(|| EstimateCommandError::RevisionSpaceExhausted(address.clone()).into())
}

#[cfg(test)]
mod tests {
    use crate::{
        command::{
            CommandOutcome, CommandRequest, CreateEdge, CreateNode, GraphCommand, RemoveEstimate,
            SetEstimate,
        },
        domain::{
            CausalEffect, Distribution, EdgePayload, EntityId, EstimateAddress, EstimateId,
            EstimateOwner, EstimateSlot, Factor, ProjectId, SignedInfluence,
        },
        project::{EstimateCommandError, ProjectCatalog, ProjectError},
    };

    fn address(project: &ProjectId, owner: EstimateOwner, id: u64) -> EstimateAddress {
        EstimateAddress::new(project.clone(), owner, EstimateId::new(id))
    }

    fn catalog() -> (ProjectCatalog, ProjectId) {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        for (revision, name) in [(0, "source"), (1, "target")] {
            catalog
                .execute(
                    &project.id,
                    CommandRequest::new(
                        revision,
                        GraphCommand::CreateNode(CreateNode {
                            name: name.to_owned(),
                            title: name.to_owned(),
                            payload: crate::domain::NodePayload::Factor(Factor {
                                current: None,
                                desired: None,
                                controllable: true,
                                evidence: vec![],
                            }),
                        }),
                    ),
                )
                .unwrap();
        }
        (catalog, project.id)
    }

    #[test]
    fn creates_replaces_shows_removes_and_replays_node_estimates() {
        let (mut catalog, project) = catalog();
        let address = address(&project, EstimateOwner::Node(EntityId::new(0)), 0);
        let request = CommandRequest::new(
            2,
            GraphCommand::SetEstimate(SetEstimate {
                address: address.clone(),
                slot: EstimateSlot::Current,
                distribution: Distribution::beta(2.0, 3.0).unwrap(),
                provenance: vec!["elicitation".to_owned()],
            }),
        );
        let first = catalog.execute(&project, request.clone()).unwrap();
        assert_eq!(first, catalog.execute(&project, request).unwrap());
        let CommandOutcome::EstimateSet(created) = first.outcome else {
            unreachable!()
        };
        assert_eq!(created.revision, 0);
        assert_eq!(catalog.get_estimate(&project, &address).unwrap(), created);
        assert_eq!(
            catalog
                .get_node(&project, EntityId::new(0))
                .unwrap()
                .unwrap()
                .revision,
            1
        );

        let replaced = catalog
            .execute(
                &project,
                CommandRequest::new(
                    3,
                    GraphCommand::SetEstimate(SetEstimate {
                        address: address.clone(),
                        slot: EstimateSlot::Current,
                        distribution: Distribution::beta(4.0, 2.0).unwrap(),
                        provenance: vec![],
                    }),
                ),
            )
            .unwrap();
        let CommandOutcome::EstimateSet(replaced) = replaced.outcome else {
            unreachable!()
        };
        assert_eq!(replaced.revision, 1);
        let removed = catalog
            .execute(
                &project,
                CommandRequest::new(
                    4,
                    GraphCommand::RemoveEstimate(RemoveEstimate {
                        address: address.clone(),
                    }),
                ),
            )
            .unwrap();
        assert!(matches!(
            removed.outcome,
            CommandOutcome::EstimateRemoved(_)
        ));
        assert_eq!(
            catalog.get_estimate(&project, &address),
            Err(ProjectError::EstimateCommand(
                EstimateCommandError::NotFound(address)
            ))
        );
    }

    #[test]
    fn rejects_invalid_support_collisions_slots_and_scope() {
        let (mut catalog, project) = catalog();
        let current = address(&project, EstimateOwner::Node(EntityId::new(0)), 0);
        catalog
            .execute(
                &project,
                CommandRequest::new(
                    2,
                    GraphCommand::SetEstimate(SetEstimate {
                        address: current.clone(),
                        slot: EstimateSlot::Current,
                        distribution: Distribution::beta(2.0, 2.0).unwrap(),
                        provenance: vec![],
                    }),
                ),
            )
            .unwrap();
        for (address, slot, distribution) in [
            (
                current.clone(),
                EstimateSlot::Desired,
                Distribution::beta(2.0, 2.0).unwrap(),
            ),
            (
                address(&project, EstimateOwner::Node(EntityId::new(0)), 1),
                EstimateSlot::Current,
                Distribution::beta(2.0, 2.0).unwrap(),
            ),
            (
                address(&project, EstimateOwner::Node(EntityId::new(1)), 0),
                EstimateSlot::Current,
                Distribution::normal(0.5, 0.1).unwrap(),
            ),
        ] {
            assert!(
                catalog
                    .execute(
                        &project,
                        CommandRequest::new(
                            3,
                            GraphCommand::SetEstimate(SetEstimate {
                                address,
                                slot,
                                distribution,
                                provenance: vec![],
                            })
                        )
                    )
                    .is_err()
            );
        }
        let foreign = address(
            &ProjectId::new("foreign").unwrap(),
            EstimateOwner::Node(EntityId::new(1)),
            0,
        );
        assert!(matches!(
            catalog.execute(
                &project,
                CommandRequest::new(
                    3,
                    GraphCommand::SetEstimate(SetEstimate {
                        address: foreign,
                        slot: EstimateSlot::Current,
                        distribution: Distribution::beta(2.0, 2.0).unwrap(),
                        provenance: vec![],
                    })
                )
            ),
            Err(ProjectError::EstimateCommand(
                EstimateCommandError::CrossProjectAddress(_)
            ))
        ));
    }

    #[test]
    fn removes_optional_lag_but_preserves_required_effect() {
        let (mut catalog, project) = catalog();
        let effect = crate::domain::Estimate::<SignedInfluence>::new(
            EstimateId::new(0),
            Distribution::scaled_beta(2.0, 2.0, -1.0, 1.0).unwrap(),
        )
        .unwrap();
        let edge = catalog
            .execute(
                &project,
                CommandRequest::new(
                    2,
                    GraphCommand::CreateEdge(CreateEdge {
                        source: EntityId::new(0),
                        destination: EntityId::new(1),
                        payload: EdgePayload::Contributes(CausalEffect {
                            effect,
                            lag: None,
                            mechanism: String::new(),
                            evidence: vec![],
                        }),
                    }),
                ),
            )
            .unwrap();
        let CommandOutcome::EdgeCreated(edge) = edge.outcome else {
            unreachable!()
        };
        let owner = EstimateOwner::Edge(edge.id());
        let lag = address(&project, owner.clone(), 1);
        catalog
            .execute(
                &project,
                CommandRequest::new(
                    3,
                    GraphCommand::SetEstimate(SetEstimate {
                        address: lag.clone(),
                        slot: EstimateSlot::Lag,
                        distribution: Distribution::log_normal(0.0, 0.5).unwrap(),
                        provenance: vec![],
                    }),
                ),
            )
            .unwrap();
        catalog
            .execute(
                &project,
                CommandRequest::new(
                    4,
                    GraphCommand::RemoveEstimate(RemoveEstimate { address: lag }),
                ),
            )
            .unwrap();
        let effect = address(&project, owner, 0);
        assert!(matches!(
            catalog.execute(
                &project,
                CommandRequest::new(
                    5,
                    GraphCommand::RemoveEstimate(RemoveEstimate { address: effect })
                )
            ),
            Err(ProjectError::EstimateCommand(
                EstimateCommandError::Required { .. }
            ))
        ));
    }

    #[test]
    fn preserves_estimates_referenced_by_project_dependence() {
        use crate::command::SetProjectDependence;
        use crate::domain::{
            CorrelationScale, GaussianCopulaCorrelation, ProjectDependenceModel,
            ResidualDependenceGroup,
        };

        let (mut catalog, project) = catalog();
        let left = address(&project, EstimateOwner::Node(EntityId::new(0)), 0);
        let right = address(&project, EstimateOwner::Node(EntityId::new(1)), 0);
        for (revision, address) in [(2, left.clone()), (3, right.clone())] {
            catalog
                .execute(
                    &project,
                    CommandRequest::new(
                        revision,
                        GraphCommand::SetEstimate(SetEstimate {
                            address,
                            slot: EstimateSlot::Current,
                            distribution: Distribution::beta(2.0, 2.0).unwrap(),
                            provenance: vec![],
                        }),
                    ),
                )
                .unwrap();
        }
        catalog
            .execute(
                &project,
                CommandRequest::new(
                    4,
                    GraphCommand::SetProjectDependence(SetProjectDependence {
                        model: ProjectDependenceModel {
                            revision: 0,
                            residual_groups: vec![ResidualDependenceGroup {
                                members: vec![left.clone(), right],
                                correlation: GaussianCopulaCorrelation {
                                    scale: CorrelationScale::Latent,
                                    matrix: vec![vec![1.0, 0.5], vec![0.5, 1.0]],
                                },
                            }],
                        },
                    }),
                ),
            )
            .unwrap();
        assert!(matches!(
            catalog.execute(
                &project,
                CommandRequest::new(
                    5,
                    GraphCommand::RemoveEstimate(RemoveEstimate { address: left }),
                ),
            ),
            Err(ProjectError::EstimateCommand(
                EstimateCommandError::ReferencedByDependence(_)
            ))
        ));
    }
}
