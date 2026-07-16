use std::collections::BTreeMap;

use crate::{
    domain::{CompiledFormula, EstimateAddress, Formula, FormulaDocument, FormulaSet, ProjectId},
    store::GraphRepository,
};

use super::{FormulaCommandError, ProjectError, catalog::ProjectEntry, formula_primitives};

pub(super) fn compile(
    entry: &mut ProjectEntry,
    formulas: &BTreeMap<EstimateAddress, Formula>,
) -> Result<BTreeMap<EstimateAddress, CompiledFormula>, ProjectError> {
    let project = entry.project.id.clone();
    let primitives = primitives(entry, &project)?;
    compile_maps(&project, &primitives, formulas).map_err(ProjectError::from)
}

pub(crate) fn compile_maps(
    project: &ProjectId,
    primitives: &BTreeMap<EstimateAddress, Formula>,
    formulas: &BTreeMap<EstimateAddress, Formula>,
) -> Result<BTreeMap<EstimateAddress, CompiledFormula>, FormulaCommandError> {
    validate_targets(project, primitives, formulas)?;
    let set = FormulaSet::new(
        primitives
            .iter()
            .map(|(address, formula)| (address.clone(), formula.clone()))
            .chain(
                formulas
                    .iter()
                    .map(|(address, formula)| (address.clone(), formula.clone())),
            ),
    )
    .map_err(FormulaCommandError::from)?;
    formulas
        .iter()
        .map(|(address, formula)| {
            set.validate(project, formula)
                .map(|compiled| (address.clone(), compiled))
                .map_err(FormulaCommandError::from)
        })
        .collect()
}

pub(super) fn definition(
    document: &FormulaDocument,
    compiled: &BTreeMap<EstimateAddress, CompiledFormula>,
    address: &EstimateAddress,
) -> Result<crate::domain::FormulaDefinition, FormulaCommandError> {
    let formula = document
        .formulas
        .get(address)
        .ok_or_else(|| FormulaCommandError::NotFound(address.clone()))?;
    Ok(crate::domain::FormulaDefinition {
        address: address.clone(),
        formula: formula.clone(),
        compiled: compiled[address].clone(),
        provenance: document
            .provenance
            .get(address)
            .cloned()
            .unwrap_or_default(),
    })
}

fn primitives(
    entry: &mut ProjectEntry,
    project: &ProjectId,
) -> Result<BTreeMap<EstimateAddress, Formula>, ProjectError> {
    let mut values = BTreeMap::new();
    for node in entry.repository.list_nodes()? {
        for (address, formula) in formula_primitives::from_node(project, &node)? {
            if values.insert(address.clone(), formula).is_some() {
                return Err(FormulaCommandError::DuplicatePrimitive(address).into());
            }
        }
    }
    for edge in entry.repository.list_edges()? {
        for (address, formula) in formula_primitives::from_edge(project, &edge) {
            if values.insert(address.clone(), formula).is_some() {
                return Err(FormulaCommandError::DuplicatePrimitive(address).into());
            }
        }
    }
    Ok(values)
}

fn validate_targets(
    project: &ProjectId,
    primitives: &BTreeMap<EstimateAddress, Formula>,
    formulas: &BTreeMap<EstimateAddress, Formula>,
) -> Result<(), FormulaCommandError> {
    for address in formulas.keys() {
        if &address.project != project {
            return Err(FormulaCommandError::CrossProjectAddress(address.clone()));
        }
        if address.components.is_empty() {
            return Err(FormulaCommandError::RootAddress(address.clone()));
        }
        let mut root = address.clone();
        root.components.clear();
        if !primitives.contains_key(&root) {
            return Err(FormulaCommandError::MissingPrimitiveRoot(root));
        }
        if address.components.len() > 1 {
            let mut parent = address.clone();
            parent.components.pop();
            if !formulas.contains_key(&parent) {
                return Err(FormulaCommandError::MissingParent(parent));
            }
        }
    }
    Ok(())
}
