use std::collections::BTreeMap;

use crate::{
    domain::{EntityId, EstimateAddress, Formula},
    project::{formula_primitives, formula_projection},
};

use super::{EntityDocument, ImportError, ProjectDocument, SourceDocument};

pub(super) fn validate(
    project: &SourceDocument<ProjectDocument>,
    entities: &BTreeMap<EntityId, SourceDocument<EntityDocument>>,
) -> Result<(), ImportError> {
    for address in project.document.formulas.provenance.keys() {
        if !project.document.formulas.formulas.contains_key(address) {
            return Err(ImportError::OrphanFormulaProvenance {
                path: project.path.clone(),
                address: address.clone(),
            });
        }
    }
    let project_id = &project.document.project.id;
    let mut primitives: BTreeMap<EstimateAddress, Formula> = BTreeMap::new();
    for entity in entities.values() {
        let values = formula_primitives::from_node(project_id, &entity.document.node)
            .map_err(|error| invalid(project, error))?
            .into_iter()
            .chain(
                entity
                    .document
                    .outgoing_edges
                    .iter()
                    .flat_map(|edge| formula_primitives::from_edge(project_id, edge)),
            );
        for (address, formula) in values {
            if primitives.insert(address.clone(), formula).is_some() {
                return Err(invalid(
                    project,
                    format!("primitive estimate address {address} occurs more than once"),
                ));
            }
        }
    }
    formula_projection::compile_maps(project_id, &primitives, &project.document.formulas.formulas)
        .map_err(|error| invalid(project, error))?;
    Ok(())
}

fn invalid(
    project: &SourceDocument<ProjectDocument>,
    error: impl std::fmt::Display,
) -> ImportError {
    ImportError::InvalidFormulas {
        path: project.path.clone(),
        message: error.to_string(),
    }
}
