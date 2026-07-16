use crate::{
    command::{CommandOutcome, RemoveFormula, SetFormula},
    domain::{EstimateAddress, FormulaCatalog, FormulaDefinition},
};

use super::{FormulaCommandError, ProjectError, catalog::ProjectEntry, formula_projection};

pub(super) fn set(
    entry: &mut ProjectEntry,
    command: SetFormula,
) -> Result<CommandOutcome, ProjectError> {
    validate_revision(entry, command.expected_revision)?;
    let mut formulas = entry.formulas.formulas.clone();
    formulas.insert(command.address.clone(), command.formula);
    let compiled = formula_projection::compile(entry, &formulas)?;
    let revision = next_revision(entry)?;
    entry.formulas.formulas = formulas;
    entry
        .formulas
        .provenance
        .insert(command.address.clone(), command.provenance);
    entry.formulas.revision = revision;
    Ok(CommandOutcome::FormulaSet(formula_projection::definition(
        &entry.formulas,
        &compiled,
        &command.address,
    )?))
}

pub(super) fn remove(
    entry: &mut ProjectEntry,
    command: RemoveFormula,
) -> Result<CommandOutcome, ProjectError> {
    validate_revision(entry, command.expected_revision)?;
    let compiled = formula_projection::compile(entry, &entry.formulas.formulas.clone())?;
    let removed = get(entry, &command.address, &compiled)?;
    for address in entry.formulas.formulas.keys() {
        if address != &command.address && is_descendant(address, &command.address) {
            return Err(FormulaCommandError::HasDescendant {
                address: command.address,
                descendant: Box::new(address.clone()),
            }
            .into());
        }
        if address != &command.address && compiled[address].dependencies.contains(&command.address)
        {
            return Err(FormulaCommandError::Referenced {
                address: command.address,
                dependent: Box::new(address.clone()),
            }
            .into());
        }
    }
    let revision = next_revision(entry)?;
    entry.formulas.formulas.remove(&command.address);
    entry.formulas.provenance.remove(&command.address);
    entry.formulas.revision = revision;
    Ok(CommandOutcome::FormulaRemoved(removed))
}

pub(super) fn list(entry: &mut ProjectEntry) -> Result<FormulaCatalog, ProjectError> {
    let compiled = formula_projection::compile(entry, &entry.formulas.formulas.clone())?;
    let formulas = entry
        .formulas
        .formulas
        .keys()
        .map(|address| get(entry, address, &compiled))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FormulaCatalog {
        revision: entry.formulas.revision,
        formulas,
    })
}

pub(super) fn show(
    entry: &mut ProjectEntry,
    address: &EstimateAddress,
) -> Result<FormulaDefinition, ProjectError> {
    let compiled = formula_projection::compile(entry, &entry.formulas.formulas.clone())?;
    get(entry, address, &compiled)
}

fn get(
    entry: &ProjectEntry,
    address: &EstimateAddress,
    compiled: &std::collections::BTreeMap<EstimateAddress, crate::domain::CompiledFormula>,
) -> Result<FormulaDefinition, ProjectError> {
    Ok(formula_projection::definition(
        &entry.formulas,
        compiled,
        address,
    )?)
}

fn validate_revision(entry: &ProjectEntry, expected: u64) -> Result<(), FormulaCommandError> {
    if entry.formulas.revision != expected {
        return Err(FormulaCommandError::RevisionConflict {
            expected,
            current: entry.formulas.revision,
        });
    }
    Ok(())
}

fn next_revision(entry: &ProjectEntry) -> Result<u64, FormulaCommandError> {
    entry
        .formulas
        .revision
        .checked_add(1)
        .ok_or(FormulaCommandError::RevisionSpaceExhausted)
}

fn is_descendant(candidate: &EstimateAddress, parent: &EstimateAddress) -> bool {
    candidate.project == parent.project
        && candidate.owner == parent.owner
        && candidate.estimate == parent.estimate
        && candidate.components.starts_with(&parent.components)
        && candidate.components.len() > parent.components.len()
}

#[cfg(test)]
mod tests {
    use crate::{
        command::{
            CommandOutcome, CommandRequest, CreateNode, GraphCommand, RemoveFormula, SetEstimate,
            SetFormula,
        },
        domain::{
            Distribution, EntityId, EstimateAddress, EstimateComponentId, EstimateId,
            EstimateOwner, EstimateSlot, Factor, Formula, NodePayload, Unit,
        },
        project::{FormulaCommandError, ProjectCatalog, ProjectError},
    };

    fn setup() -> (ProjectCatalog, crate::domain::ProjectId, EstimateAddress) {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    0,
                    GraphCommand::CreateNode(CreateNode {
                        name: "flow".to_owned(),
                        title: "Flow".to_owned(),
                        payload: NodePayload::Factor(Factor {
                            current: None,
                            desired: None,
                            controllable: true,
                            evidence: vec![],
                        }),
                    }),
                ),
            )
            .unwrap();
        let root = EstimateAddress::new(
            project.id.clone(),
            EstimateOwner::Node(EntityId::new(0)),
            EstimateId::new(0),
        );
        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    1,
                    GraphCommand::SetEstimate(SetEstimate {
                        address: root.clone(),
                        slot: EstimateSlot::Current,
                        distribution: Distribution::beta(2.0, 2.0).unwrap(),
                        provenance: vec![],
                    }),
                ),
            )
            .unwrap();
        (catalog, project.id, root)
    }

    fn component(root: &EstimateAddress, name: &str) -> EstimateAddress {
        root.clone()
            .with_component(EstimateComponentId::new(name).unwrap())
    }

    fn literal(value: f64) -> Formula {
        Formula::Literal {
            distribution: Distribution::point(value).unwrap(),
            unit: Unit::dimensionless(),
        }
    }

    #[test]
    fn stores_lists_replaces_and_removes_validated_components() {
        let (mut catalog, project, root) = setup();
        let address = component(&root, "baseline");
        let request = CommandRequest::new(
            2,
            GraphCommand::SetFormula(SetFormula {
                address: address.clone(),
                formula: Formula::Reference { address: root },
                expected_revision: 0,
                provenance: vec!["decomposition".to_owned()],
            }),
        );
        let first = catalog.execute(&project, request.clone()).unwrap();
        assert_eq!(first, catalog.execute(&project, request).unwrap());
        let CommandOutcome::FormulaSet(created) = first.outcome else {
            unreachable!()
        };
        assert!(created.compiled.unit.is_dimensionless());
        assert_eq!(created.compiled.dependencies.len(), 1);
        assert_eq!(created.provenance, vec!["decomposition"]);

        let replaced = catalog
            .execute(
                &project,
                CommandRequest::new(
                    3,
                    GraphCommand::SetFormula(SetFormula {
                        address: address.clone(),
                        formula: literal(0.5),
                        expected_revision: 1,
                        provenance: vec![],
                    }),
                ),
            )
            .unwrap();
        assert!(matches!(replaced.outcome, CommandOutcome::FormulaSet(_)));
        let removed = catalog
            .execute(
                &project,
                CommandRequest::new(
                    4,
                    GraphCommand::RemoveFormula(RemoveFormula {
                        address,
                        expected_revision: 2,
                    }),
                ),
            )
            .unwrap();
        assert!(matches!(removed.outcome, CommandOutcome::FormulaRemoved(_)));
    }

    #[test]
    fn rejects_missing_roots_parents_unit_mismatch_and_stale_revisions() {
        let (mut catalog, project, root) = setup();
        let address = component(&root, "base");
        let missing = component(
            &EstimateAddress::new(
                project.clone(),
                EstimateOwner::Node(EntityId::new(1)),
                EstimateId::new(0),
            ),
            "missing",
        );
        assert!(
            catalog
                .execute(
                    &project,
                    CommandRequest::new(
                        2,
                        GraphCommand::SetFormula(SetFormula {
                            address: missing,
                            formula: literal(1.0),
                            expected_revision: 0,
                            provenance: vec![],
                        })
                    )
                )
                .is_err()
        );
        let nested = address
            .clone()
            .with_component(EstimateComponentId::new("child").unwrap());
        assert!(matches!(
            catalog.execute(
                &project,
                CommandRequest::new(
                    2,
                    GraphCommand::SetFormula(SetFormula {
                        address: nested,
                        formula: literal(1.0),
                        expected_revision: 0,
                        provenance: vec![],
                    })
                )
            ),
            Err(ProjectError::FormulaCommand(
                FormulaCommandError::MissingParent(_)
            ))
        ));
        catalog
            .execute(
                &project,
                CommandRequest::new(
                    2,
                    GraphCommand::SetFormula(SetFormula {
                        address: address.clone(),
                        formula: literal(1.0),
                        expected_revision: 0,
                        provenance: vec![],
                    }),
                ),
            )
            .unwrap();
        assert!(matches!(
            catalog.execute(
                &project,
                CommandRequest::new(
                    3,
                    GraphCommand::SetFormula(SetFormula {
                        address,
                        formula: literal(2.0),
                        expected_revision: 0,
                        provenance: vec![],
                    })
                )
            ),
            Err(ProjectError::FormulaCommand(
                FormulaCommandError::RevisionConflict { .. }
            ))
        ));
    }

    #[test]
    fn rejects_formula_cycles_and_additive_unit_mismatches() {
        let (mut catalog, project, root) = setup();
        let cyclic = component(&root, "cyclic");
        assert!(matches!(
            catalog.execute(
                &project,
                CommandRequest::new(
                    2,
                    GraphCommand::SetFormula(SetFormula {
                        address: cyclic.clone(),
                        formula: Formula::Reference { address: cyclic },
                        expected_revision: 0,
                        provenance: vec![],
                    }),
                ),
            ),
            Err(ProjectError::FormulaCommand(FormulaCommandError::Formula(
                crate::domain::FormulaError::ReferenceCycle(_)
            )))
        ));

        let mismatch = component(&root, "mismatch");
        let formula = Formula::Sum {
            terms: vec![
                Formula::Reference { address: root },
                Formula::Literal {
                    distribution: Distribution::point(1.0).unwrap(),
                    unit: Unit::base("usd").unwrap(),
                },
            ],
        };
        assert!(matches!(
            catalog.execute(
                &project,
                CommandRequest::new(
                    2,
                    GraphCommand::SetFormula(SetFormula {
                        address: mismatch,
                        formula,
                        expected_revision: 0,
                        provenance: vec![],
                    }),
                ),
            ),
            Err(ProjectError::FormulaCommand(FormulaCommandError::Formula(
                crate::domain::FormulaError::UnitMismatch { .. }
            )))
        ));
    }

    #[test]
    fn refuses_removal_while_descendants_or_references_exist() {
        let (mut catalog, project, root) = setup();
        let parent = component(&root, "parent");
        catalog
            .execute(
                &project,
                CommandRequest::new(
                    2,
                    GraphCommand::SetFormula(SetFormula {
                        address: parent.clone(),
                        formula: literal(1.0),
                        expected_revision: 0,
                        provenance: vec![],
                    }),
                ),
            )
            .unwrap();
        let child = parent
            .clone()
            .with_component(EstimateComponentId::new("child").unwrap());
        catalog
            .execute(
                &project,
                CommandRequest::new(
                    3,
                    GraphCommand::SetFormula(SetFormula {
                        address: child,
                        formula: Formula::Reference {
                            address: parent.clone(),
                        },
                        expected_revision: 1,
                        provenance: vec![],
                    }),
                ),
            )
            .unwrap();
        assert!(matches!(
            catalog.execute(
                &project,
                CommandRequest::new(
                    4,
                    GraphCommand::RemoveFormula(RemoveFormula {
                        address: parent,
                        expected_revision: 2,
                    })
                )
            ),
            Err(ProjectError::FormulaCommand(
                FormulaCommandError::HasDescendant { .. }
            ))
        ));
    }
}
