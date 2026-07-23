use crate::{
    command::{CommandOutcome, RemoveEstimate, SetSquiggleEstimate},
    domain::{
        EstimateAddress, EstimateOwner, EstimateSource, PrimitiveEstimate, ProjectId,
        assess_squiggle_estimate,
    },
    store::{GraphRepository, RepositoryError},
};

use super::{
    EstimateCommandError, ProjectError, catalog::ProjectEntry, estimate_edge, estimate_node,
    estimate_node_find, estimate_node_remove, estimate_support,
};

pub(super) fn set_squiggle(
    entry: &mut ProjectEntry,
    command: SetSquiggleEstimate,
) -> Result<CommandOutcome, ProjectError> {
    validate_address(&entry.project.id, &command.address)?;
    let slot = command
        .slot
        .validated()
        .map_err(EstimateCommandError::from)?;
    let (_, expected_unit) = estimate_target(entry, &command.address, &slot)?;
    let (definition, _, distribution) =
        assess_squiggle_estimate(command.definition, &expected_unit)
            .map_err(EstimateCommandError::from)?;
    let source = EstimateSource::Squiggle {
        definition: Box::new(definition),
    };
    set_value(
        entry,
        command.address,
        slot,
        distribution,
        source,
        command.provenance,
        command.uncertainty,
    )
    .map(CommandOutcome::SquiggleEstimateSet)
}

fn estimate_target(
    entry: &mut ProjectEntry,
    address: &EstimateAddress,
    slot: &crate::domain::EstimateSlot,
) -> Result<(crate::domain::SquiggleEstimateSupport, crate::domain::Unit), ProjectError> {
    if let EstimateOwner::Edge(id) = &address.owner
        && matches!(slot, crate::domain::EstimateSlot::Response)
    {
        let edge = entry
            .repository
            .get_edge(id)?
            .ok_or_else(|| RepositoryError::MissingEdge(id.to_string()))?;
        let response = match edge.payload {
            crate::domain::EdgePayload::Contributes(value) => value.response,
            _ => {
                return Err(EstimateCommandError::InvalidSlot {
                    address: address.clone(),
                    slot: slot.clone(),
                }
                .into());
            }
        };
        return Ok((
            crate::domain::SquiggleEstimateSupport::Real,
            response.destination_unit,
        ));
    }
    let EstimateOwner::Node(id) = &address.owner else {
        return Ok((
            slot.estimate_support(),
            slot.unit().map_err(EstimateCommandError::from)?,
        ));
    };
    let node = entry
        .repository
        .get_node(*id)?
        .ok_or(RepositoryError::MissingEntity(*id))?;
    if let Some(state) = node.native_state {
        if !matches!(
            slot,
            crate::domain::EstimateSlot::Current | crate::domain::EstimateSlot::Forecast
        ) {
            return Err(EstimateCommandError::InvalidSlot {
                address: address.clone(),
                slot: slot.clone(),
            }
            .into());
        }
        return state
            .quantity
            .estimate_target()
            .map_err(EstimateCommandError::from)
            .map_err(ProjectError::from);
    }
    if let crate::domain::NodePayload::Metric(metric) = node.payload {
        if !matches!(slot, crate::domain::EstimateSlot::Current) {
            return Err(EstimateCommandError::InvalidSlot {
                address: address.clone(),
                slot: slot.clone(),
            }
            .into());
        }
        return metric
            .quantity
            .estimate_target()
            .map_err(EstimateCommandError::from)
            .map_err(ProjectError::from);
    }
    Ok((
        slot.estimate_support(),
        slot.unit().map_err(EstimateCommandError::from)?,
    ))
}

fn set_value(
    entry: &mut ProjectEntry,
    address: EstimateAddress,
    slot: crate::domain::EstimateSlot,
    distribution: crate::domain::Distribution,
    source: EstimateSource,
    provenance: Vec<String>,
    uncertainty: crate::domain::EstimateUncertainty,
) -> Result<PrimitiveEstimate, ProjectError> {
    validate_address(&entry.project.id, &address)?;
    let slot = slot.validated().map_err(EstimateCommandError::from)?;
    let metadata = estimate_support::EstimateMetadata {
        source,
        provenance,
        uncertainty,
    };
    let value = match &address.owner {
        EstimateOwner::Node(id) => {
            let mut node = entry
                .repository
                .get_node(*id)?
                .ok_or(RepositoryError::MissingEntity(*id))?;
            let value = estimate_node::set(&mut node, &address, slot, distribution, metadata)?;
            node.revision = next_owner_revision(node.revision, &address)?;
            entry.repository.update_node(node)?;
            value
        }
        EstimateOwner::Edge(id) => {
            let mut edge = entry
                .repository
                .get_edge(id)?
                .ok_or_else(|| RepositoryError::MissingEdge(id.to_string()))?;
            let value = estimate_edge::set(&mut edge, &address, slot, distribution, metadata)?;
            edge.revision = next_owner_revision(edge.revision, &address)?;
            entry.repository.update_edge(edge)?;
            value
        }
    };
    Ok(value)
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
            SetNodeQuantityState, SetSquiggleEstimate,
        },
        domain::{
            CausalEffect, Distribution, EdgePayload, EntityId, EstimateAddress, EstimateId,
            EstimateOwner, EstimateSlot, EstimateSource, EstimateUncertainty, Factor,
            LinearResponse, Metric, NodePayload, ProjectId, QuantityDefinition, QuantitySupport,
            QuantityValue, SquiggleEstimateDefinition, Unit,
        },
        project::{EstimateCommandError, ProjectCatalog, ProjectError},
    };

    fn address(project: &ProjectId, owner: EstimateOwner, id: u64) -> EstimateAddress {
        EstimateAddress::new(project.clone(), owner, EstimateId::new(id))
    }

    fn definition(source: &str, target_unit: Unit) -> SquiggleEstimateDefinition {
        SquiggleEstimateDefinition {
            source: source.to_owned(),
            seed: 42,
            sample_count: 256,
            target_unit,
        }
    }

    fn set_squiggle(
        address: EstimateAddress,
        slot: EstimateSlot,
        source: &str,
        target_unit: Unit,
    ) -> GraphCommand {
        GraphCommand::SetSquiggleEstimate(SetSquiggleEstimate {
            address,
            slot,
            definition: definition(source, target_unit),
            provenance: vec![],
            uncertainty: EstimateUncertainty::default(),
        })
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
                                controllable: true,
                                evidence: vec![],
                            }),
                        }),
                    ),
                )
                .unwrap();
        }
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
        for (revision, node) in [(2, 0), (3, 1)] {
            catalog
                .execute(
                    &project.id,
                    CommandRequest::new(
                        revision,
                        GraphCommand::SetNodeQuantityState(SetNodeQuantityState {
                            node: EntityId::new(node),
                            expected_revision: 0,
                            quantity: quantity.clone(),
                        }),
                    ),
                )
                .unwrap();
        }
        (catalog, project.id)
    }

    fn metric_catalog(support: QuantitySupport) -> (ProjectCatalog, ProjectId) {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        let quantity =
            QuantityDefinition::new("days", Some("p95 weekly".to_owned()), support).unwrap();
        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    0,
                    GraphCommand::CreateNode(CreateNode {
                        name: "lead_time".to_owned(),
                        title: "Lead time".to_owned(),
                        payload: NodePayload::Metric(
                            Metric::with_quantity(quantity, None).unwrap(),
                        ),
                    }),
                ),
            )
            .unwrap();
        (catalog, project.id)
    }

    #[test]
    fn creates_replaces_shows_removes_and_replays_node_estimates() {
        let (mut catalog, project) = catalog();
        let address = address(&project, EstimateOwner::Node(EntityId::new(0)), 0);
        let request = CommandRequest::new(
            4,
            GraphCommand::SetSquiggleEstimate(SetSquiggleEstimate {
                address: address.clone(),
                slot: EstimateSlot::Current,
                definition: definition("beta(2, 3)", Unit::dimensionless()),
                provenance: vec!["elicitation".to_owned()],
                uncertainty: EstimateUncertainty::new(
                    "limited evidence",
                    "daily variation",
                    "survey sampling error",
                )
                .unwrap(),
            }),
        );
        let first = catalog.execute(&project, request.clone()).unwrap();
        assert_eq!(first, catalog.execute(&project, request).unwrap());
        let CommandOutcome::SquiggleEstimateSet(created) = first.outcome else {
            unreachable!()
        };
        assert_eq!(created.revision, 0);
        assert_eq!(
            created
                .quantity
                .as_ref()
                .map(|quantity| quantity.unit.as_str()),
            Some("state")
        );
        assert_eq!(created.uncertainty.process, "daily variation");
        assert_eq!(catalog.get_estimate(&project, &address).unwrap(), created);
        assert_eq!(
            catalog
                .get_node(&project, EntityId::new(0))
                .unwrap()
                .unwrap()
                .revision,
            2
        );

        let replaced = catalog
            .execute(
                &project,
                CommandRequest::new(
                    5,
                    GraphCommand::SetSquiggleEstimate(SetSquiggleEstimate {
                        address: address.clone(),
                        slot: EstimateSlot::Current,
                        definition: definition("beta(4, 2)", Unit::dimensionless()),
                        provenance: vec![],
                        uncertainty: EstimateUncertainty::default(),
                    }),
                ),
            )
            .unwrap();
        let CommandOutcome::SquiggleEstimateSet(replaced) = replaced.outcome else {
            unreachable!()
        };
        assert_eq!(replaced.revision, 1);
        let removed = catalog
            .execute(
                &project,
                CommandRequest::new(
                    6,
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
    fn persists_squiggle_sources_and_preserves_symbolic_distributions() {
        let (mut catalog, project) = catalog();
        let address = address(&project, EstimateOwner::Node(EntityId::new(0)), 0);
        let request = CommandRequest::new(
            4,
            GraphCommand::SetSquiggleEstimate(SetSquiggleEstimate {
                address: address.clone(),
                slot: EstimateSlot::Current,
                definition: SquiggleEstimateDefinition {
                    source: "beta(8, 2)".to_owned(),
                    seed: 42,
                    sample_count: 512,
                    target_unit: Unit::dimensionless(),
                },
                provenance: vec!["direct Squiggle model".to_owned()],
                uncertainty: EstimateUncertainty::default(),
            }),
        );
        let first = catalog.execute(&project, request.clone()).unwrap();
        assert_eq!(first, catalog.execute(&project, request).unwrap());
        let CommandOutcome::SquiggleEstimateSet(created) = first.outcome else {
            panic!("expected Squiggle estimate")
        };
        assert!(matches!(created.source, EstimateSource::Squiggle { .. }));
        assert_eq!(
            serde_json::to_value(&created.distribution).unwrap()["type"],
            "beta"
        );
        assert_eq!(catalog.get_estimate(&project, &address).unwrap(), created);

        let invalid = catalog.execute(
            &project,
            CommandRequest::new(
                5,
                GraphCommand::SetSquiggleEstimate(SetSquiggleEstimate {
                    address,
                    slot: EstimateSlot::Current,
                    definition: SquiggleEstimateDefinition {
                        source: "normal(0.5, 10)".to_owned(),
                        seed: 42,
                        sample_count: 512,
                        target_unit: Unit::dimensionless(),
                    },
                    provenance: vec![],
                    uncertainty: EstimateUncertainty::default(),
                }),
            ),
        );
        assert!(invalid.is_err());
    }

    #[test]
    fn authors_native_metric_estimates_without_normalizing_their_values() {
        let (mut catalog, project) = metric_catalog(QuantitySupport::NonNegative);
        let address = address(&project, EstimateOwner::Node(EntityId::new(0)), 0);
        let created = catalog
            .execute(
                &project,
                CommandRequest::new(
                    1,
                    set_squiggle(
                        address.clone(),
                        EstimateSlot::Current,
                        "lognormal(2, 0.3)",
                        Unit::base("days").unwrap(),
                    ),
                ),
            )
            .unwrap();
        let CommandOutcome::SquiggleEstimateSet(created) = created.outcome else {
            panic!("expected metric estimate")
        };
        assert_eq!(catalog.get_estimate(&project, &address).unwrap(), created);

        catalog
            .execute(
                &project,
                CommandRequest::new(
                    2,
                    GraphCommand::RemoveEstimate(RemoveEstimate {
                        address: address.clone(),
                    }),
                ),
            )
            .unwrap();
        assert!(matches!(
            catalog.get_estimate(&project, &address),
            Err(ProjectError::EstimateCommand(
                EstimateCommandError::NotFound(_)
            ))
        ));
    }

    #[test]
    fn rejects_metric_distributions_outside_support_and_persists_native_squiggle_sources() {
        let (mut catalog, project) = metric_catalog(QuantitySupport::Bounded {
            lower: 0.0,
            upper: 30.0,
        });
        let address = address(&project, EstimateOwner::Node(EntityId::new(0)), 0);
        assert!(matches!(
            catalog.execute(
                &project,
                CommandRequest::new(
                    1,
                    set_squiggle(
                        address.clone(),
                        EstimateSlot::Current,
                        "normal(5, 100)",
                        Unit::base("days").unwrap(),
                    ),
                ),
            ),
            Err(ProjectError::EstimateCommand(
                EstimateCommandError::Estimate(_) | EstimateCommandError::Quantity(_)
            ))
        ));

        let result = catalog
            .execute(
                &project,
                CommandRequest::new(
                    1,
                    set_squiggle(
                        address.clone(),
                        EstimateSlot::Current,
                        "30 * beta(2, 5)",
                        Unit::base("days").unwrap(),
                    ),
                ),
            )
            .unwrap();
        let CommandOutcome::SquiggleEstimateSet(created) = result.outcome else {
            panic!("expected native Squiggle estimate")
        };
        assert!(matches!(created.source, EstimateSource::Squiggle { .. }));
        assert_eq!(catalog.get_estimate(&project, &address).unwrap(), created);
    }

    #[test]
    fn rejects_invalid_support_collisions_slots_and_scope() {
        let (mut catalog, project) = catalog();
        let current = address(&project, EstimateOwner::Node(EntityId::new(0)), 0);
        catalog
            .execute(
                &project,
                CommandRequest::new(
                    4,
                    set_squiggle(
                        current.clone(),
                        EstimateSlot::Current,
                        "beta(2, 2)",
                        Unit::dimensionless(),
                    ),
                ),
            )
            .unwrap();
        for (address, slot, source) in [
            (current.clone(), EstimateSlot::Forecast, "beta(2, 2)"),
            (
                address(&project, EstimateOwner::Node(EntityId::new(0)), 1),
                EstimateSlot::Current,
                "beta(2, 2)",
            ),
            (
                address(&project, EstimateOwner::Node(EntityId::new(1)), 0),
                EstimateSlot::Current,
                "normal(0.5, 10)",
            ),
        ] {
            assert!(
                catalog
                    .execute(
                        &project,
                        CommandRequest::new(
                            5,
                            set_squiggle(address, slot, source, Unit::dimensionless())
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
                    5,
                    set_squiggle(
                        foreign,
                        EstimateSlot::Current,
                        "beta(2, 2)",
                        Unit::dimensionless(),
                    )
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
        let response = crate::domain::Estimate::<QuantityValue>::new(
            EstimateId::new(0),
            Distribution::point(0.5).unwrap(),
        )
        .unwrap();
        let edge = catalog
            .execute(
                &project,
                CommandRequest::new(
                    4,
                    GraphCommand::CreateEdge(CreateEdge {
                        source: EntityId::new(0),
                        destination: EntityId::new(1),
                        payload: EdgePayload::Contributes(
                            CausalEffect::linear(
                                LinearResponse {
                                    source_change: 1.0,
                                    source_unit: Unit::dimensionless(),
                                    destination_change: response,
                                    destination_unit: Unit::dimensionless(),
                                },
                                None,
                                String::new(),
                                vec![],
                            )
                            .unwrap(),
                        ),
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
                    5,
                    set_squiggle(
                        lag.clone(),
                        EstimateSlot::Lag,
                        "lognormal(0, 0.5)",
                        Unit::base("duration").unwrap(),
                    ),
                ),
            )
            .unwrap();
        catalog
            .execute(
                &project,
                CommandRequest::new(
                    6,
                    GraphCommand::RemoveEstimate(RemoveEstimate { address: lag }),
                ),
            )
            .unwrap();
        let response = address(&project, owner, 0);
        assert!(matches!(
            catalog.execute(
                &project,
                CommandRequest::new(
                    7,
                    GraphCommand::RemoveEstimate(RemoveEstimate { address: response })
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
        for (revision, address) in [(4, left.clone()), (5, right.clone())] {
            catalog
                .execute(
                    &project,
                    CommandRequest::new(
                        revision,
                        set_squiggle(
                            address,
                            EstimateSlot::Current,
                            "beta(2, 2)",
                            Unit::dimensionless(),
                        ),
                    ),
                )
                .unwrap();
        }
        catalog
            .execute(
                &project,
                CommandRequest::new(
                    6,
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
                    7,
                    GraphCommand::RemoveEstimate(RemoveEstimate { address: left }),
                ),
            ),
            Err(ProjectError::EstimateCommand(
                EstimateCommandError::ReferencedByDependence(_)
            ))
        ));
    }
}
