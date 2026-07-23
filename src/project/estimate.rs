use crate::{
    command::{CommandOutcome, RemoveEstimate, SetEstimate, SetFermiEstimate, SetSquiggleEstimate},
    domain::{
        EstimateAddress, EstimateOwner, EstimateSource, PrimitiveEstimate, ProjectId, assess_fermi,
        assess_squiggle_estimate,
    },
    store::{GraphRepository, RepositoryError},
};

use super::{
    EstimateCommandError, ProjectError, catalog::ProjectEntry, estimate_edge, estimate_node,
    estimate_node_find, estimate_node_remove, estimate_support,
};

pub(super) fn set(
    entry: &mut ProjectEntry,
    command: SetEstimate,
) -> Result<CommandOutcome, ProjectError> {
    set_value(
        entry,
        command.address,
        command.slot,
        command.distribution,
        EstimateSource::Distribution,
        command.provenance,
        command.uncertainty,
    )
    .map(CommandOutcome::EstimateSet)
}

pub(super) fn set_fermi(
    entry: &mut ProjectEntry,
    command: SetFermiEstimate,
) -> Result<CommandOutcome, ProjectError> {
    validate_address(&entry.project.id, &command.address)?;
    let slot = command
        .slot
        .validated()
        .map_err(EstimateCommandError::from)?;
    let (support, expected_unit) = fermi_target(entry, &command.address, &slot)?;
    let definition = command
        .definition
        .validated()
        .map_err(EstimateCommandError::from)?;
    let assessment = assess_fermi(
        &entry.project.id,
        definition.formula.clone(),
        support,
        expected_unit,
        definition.monte_carlo,
    )
    .map_err(EstimateCommandError::from)?;
    let distribution = assessment
        .recommended_distribution()
        .cloned()
        .ok_or(EstimateCommandError::UnavailableFermiRecommendation)?;
    let source = EstimateSource::Fermi {
        definition: Box::new(definition),
        assessment: Box::new(assessment),
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
    .map(CommandOutcome::FermiEstimateSet)
}

pub(super) fn set_squiggle(
    entry: &mut ProjectEntry,
    command: SetSquiggleEstimate,
) -> Result<CommandOutcome, ProjectError> {
    validate_address(&entry.project.id, &command.address)?;
    let slot = command
        .slot
        .validated()
        .map_err(EstimateCommandError::from)?;
    let (_, expected_unit) = fermi_target(entry, &command.address, &slot)?;
    let (definition, assessment, distribution) =
        assess_squiggle_estimate(command.definition, &expected_unit)
            .map_err(EstimateCommandError::from)?;
    let source = EstimateSource::Squiggle {
        definition: Box::new(definition),
        assessment: Box::new(assessment),
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

fn fermi_target(
    entry: &mut ProjectEntry,
    address: &EstimateAddress,
    slot: &crate::domain::EstimateSlot,
) -> Result<(crate::domain::FermiEstimateSupport, crate::domain::Unit), ProjectError> {
    if let EstimateOwner::Edge(id) = &address.owner
        && matches!(slot, crate::domain::EstimateSlot::Response)
    {
        let edge = entry
            .repository
            .get_edge(id)?
            .ok_or_else(|| RepositoryError::MissingEdge(id.to_string()))?;
        let response = match edge.payload {
            crate::domain::EdgePayload::Contributes(value) => value
                .linear_response()
                .cloned()
                .ok_or_else(|| EstimateCommandError::InvalidSlot {
                    address: address.clone(),
                    slot: slot.clone(),
                })?,
            _ => {
                return Err(EstimateCommandError::InvalidSlot {
                    address: address.clone(),
                    slot: slot.clone(),
                }
                .into());
            }
        };
        return Ok((
            crate::domain::FermiEstimateSupport::Real,
            response.destination_unit,
        ));
    }
    let EstimateOwner::Node(id) = &address.owner else {
        return Ok((
            slot.fermi_support(),
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
            crate::domain::EstimateSlot::Current | crate::domain::EstimateSlot::Desired
        ) {
            return Err(EstimateCommandError::InvalidSlot {
                address: address.clone(),
                slot: slot.clone(),
            }
            .into());
        }
        return state
            .quantity
            .fermi_target()
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
            .fermi_target()
            .map_err(EstimateCommandError::from)
            .map_err(ProjectError::from);
    }
    Ok((
        slot.fermi_support(),
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
    if let Some(formula) = super::estimate_formula_references::find(entry, &command.address) {
        return Err(EstimateCommandError::ReferencedByFormula {
            address: command.address,
            formula: Box::new(formula),
        }
        .into());
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
            SetEstimate, SetFermiEstimate, SetSquiggleEstimate,
        },
        domain::{
            CausalEffect, Distribution, EdgePayload, EntityId, EstimateAddress, EstimateId,
            EstimateOwner, EstimateSlot, EstimateSource, EstimateUncertainty, Factor,
            FermiEstimateDefinition, FermiExpressionLanguage, FermiVariable,
            FermiVariableUncertainty, Formula, Metric, MonteCarloConfig, NodePayload, ProjectId,
            QuantityDefinition, QuantitySupport, SignedInfluence, SquiggleEstimateDefinition, Unit,
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
            2,
            GraphCommand::SetEstimate(SetEstimate {
                address: address.clone(),
                slot: EstimateSlot::Current,
                distribution: Distribution::beta(2.0, 3.0).unwrap(),
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
        let CommandOutcome::EstimateSet(created) = first.outcome else {
            unreachable!()
        };
        assert_eq!(created.revision, 0);
        assert_eq!(
            created
                .quantity
                .as_ref()
                .map(|quantity| quantity.unit.as_str()),
            Some("standardized_state")
        );
        assert_eq!(created.uncertainty.process, "daily variation");
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
                        uncertainty: EstimateUncertainty::default(),
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
    fn persists_backend_evaluated_squiggle_sources_and_effective_samples() {
        let (mut catalog, project) = catalog();
        let address = address(&project, EstimateOwner::Node(EntityId::new(0)), 0);
        let request = CommandRequest::new(
            2,
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
            "empirical"
        );
        assert_eq!(catalog.get_estimate(&project, &address).unwrap(), created);

        let invalid = catalog.execute(
            &project,
            CommandRequest::new(
                3,
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
        assert!(matches!(
            invalid,
            Err(ProjectError::EstimateCommand(
                EstimateCommandError::Estimate(_)
            ))
        ));
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
                    GraphCommand::SetEstimate(SetEstimate {
                        address: address.clone(),
                        slot: EstimateSlot::Current,
                        distribution: Distribution::log_normal(2.0, 0.3).unwrap(),
                        provenance: vec!["weekly telemetry".to_owned()],
                        uncertainty: EstimateUncertainty::default(),
                    }),
                ),
            )
            .unwrap();
        let CommandOutcome::EstimateSet(created) = created.outcome else {
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
    fn rejects_metric_distributions_outside_support_and_persists_native_fermi_sources() {
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
                    GraphCommand::SetEstimate(SetEstimate {
                        address: address.clone(),
                        slot: EstimateSlot::Current,
                        distribution: Distribution::normal(5.0, 1.0).unwrap(),
                        provenance: vec![],
                        uncertainty: EstimateUncertainty::default(),
                    }),
                ),
            ),
            Err(ProjectError::EstimateCommand(
                EstimateCommandError::Quantity(_)
            ))
        ));

        let definition = FermiEstimateDefinition {
            language: FermiExpressionLanguage::OptimistSquiggleV1,
            equation: "lead_time".to_owned(),
            variables: vec![FermiVariable {
                name: "lead_time".to_owned(),
                estimate: 5.0,
                unit: "days".to_owned(),
                uncertainty: FermiVariableUncertainty::OrderOfMagnitude,
            }],
            formula: Formula::Bounded {
                input: Box::new(Formula::Literal {
                    distribution: Distribution::log_normal(5.0_f64.ln(), 0.3).unwrap(),
                    unit: Unit::base("days").unwrap(),
                }),
                lower: 0.0,
                upper: 30.0,
            },
            monte_carlo: MonteCarloConfig::new(42, 100, 1_000, 0.01, 0.01).unwrap(),
        };
        let result = catalog
            .execute(
                &project,
                CommandRequest::new(
                    1,
                    GraphCommand::SetFermiEstimate(SetFermiEstimate {
                        address: address.clone(),
                        slot: EstimateSlot::Current,
                        definition,
                        provenance: vec![],
                        uncertainty: EstimateUncertainty::default(),
                    }),
                ),
            )
            .unwrap();
        let CommandOutcome::FermiEstimateSet(created) = result.outcome else {
            panic!("expected native Fermi estimate")
        };
        assert!(matches!(created.source, EstimateSource::Fermi { .. }));
        assert!(matches!(
            serde_json::to_value(&created.distribution).unwrap()["type"].as_str(),
            Some("scaled_beta")
        ));
        assert_eq!(catalog.get_estimate(&project, &address).unwrap(), created);
    }

    #[test]
    fn persists_fermi_sources_and_replaces_them_exclusively() {
        let (mut catalog, project) = catalog();
        let address = address(&project, EstimateOwner::Node(EntityId::new(0)), 0);
        let formula = Formula::Product {
            factors: vec![
                Formula::Literal {
                    distribution: Distribution::scaled_beta(3.0, 3.0, 0.5, 0.9).unwrap(),
                    unit: Unit::dimensionless(),
                },
                Formula::Literal {
                    distribution: Distribution::scaled_beta(4.0, 2.0, 0.6, 1.0).unwrap(),
                    unit: Unit::dimensionless(),
                },
            ],
        };
        let definition = FermiEstimateDefinition {
            language: FermiExpressionLanguage::OptimistSquiggleV1,
            equation: "adoption * completion".to_owned(),
            variables: vec![
                FermiVariable {
                    name: "adoption".to_owned(),
                    estimate: 0.7,
                    unit: String::new(),
                    uncertainty: FermiVariableUncertainty::ThreePoint {
                        low: 0.5,
                        high: 0.9,
                    },
                },
                FermiVariable {
                    name: "completion".to_owned(),
                    estimate: 0.85,
                    unit: String::new(),
                    uncertainty: FermiVariableUncertainty::ThreePoint {
                        low: 0.6,
                        high: 1.0,
                    },
                },
            ],
            formula,
            monte_carlo: MonteCarloConfig::new(42, 1_000, 10_000, 0.001, 0.01).unwrap(),
        };
        let created = catalog
            .execute(
                &project,
                CommandRequest::new(
                    2,
                    GraphCommand::SetFermiEstimate(SetFermiEstimate {
                        address: address.clone(),
                        slot: EstimateSlot::Current,
                        definition,
                        provenance: vec!["planning workshop".to_owned()],
                        uncertainty: EstimateUncertainty::default(),
                    }),
                ),
            )
            .unwrap();
        let CommandOutcome::FermiEstimateSet(created) = created.outcome else {
            panic!("expected Fermi estimate result")
        };
        let EstimateSource::Fermi {
            definition,
            assessment,
        } = &created.source
        else {
            panic!("expected persisted Fermi source")
        };
        assert_eq!(definition.equation, "adoption * completion");
        assert!(assessment.recommended_distribution().is_some());
        assert_eq!(catalog.get_estimate(&project, &address).unwrap(), created);

        let replaced = catalog
            .execute(
                &project,
                CommandRequest::new(
                    3,
                    GraphCommand::SetEstimate(SetEstimate {
                        address: address.clone(),
                        slot: EstimateSlot::Current,
                        distribution: Distribution::beta(8.0, 2.0).unwrap(),
                        provenance: vec!["direct prior".to_owned()],
                        uncertainty: EstimateUncertainty::default(),
                    }),
                ),
            )
            .unwrap();
        let CommandOutcome::EstimateSet(replaced) = replaced.outcome else {
            panic!("expected direct estimate result")
        };
        assert_eq!(replaced.revision, 1);
        assert_eq!(replaced.source, EstimateSource::Distribution);
        assert_eq!(replaced.provenance, vec!["direct prior"]);
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
                        uncertainty: EstimateUncertainty::default(),
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
                                uncertainty: EstimateUncertainty::default(),
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
                        uncertainty: EstimateUncertainty::default(),
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
                        payload: EdgePayload::Contributes(CausalEffect::normalized(
                            effect,
                            None,
                            String::new(),
                            vec![],
                        )),
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
                        uncertainty: EstimateUncertainty::default(),
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
                            uncertainty: EstimateUncertainty::default(),
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
