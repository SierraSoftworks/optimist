use std::collections::BTreeMap;

use crate::domain::{EntityId, ScenarioId};

use super::{MergeAction, MergeConflict, ValidatedImport, merge_compare};

/// Deterministic, non-mutating merge report for one validated import snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MergePlan {
    /// Proposed action for `_project.md` metadata and dependence.
    pub project: MergeAction,
    /// Imported entity actions ordered by project-local ID.
    pub entities: BTreeMap<EntityId, MergeAction>,
    /// Imported scenario actions ordered by project-local ID.
    pub scenarios: BTreeMap<ScenarioId, MergeAction>,
}

impl MergePlan {
    /// Compares an import with the current immutable snapshot without mutation.
    pub fn between(current: &ValidatedImport, imported: &ValidatedImport) -> Self {
        let current_project = &current.project.document.project;
        let imported_project = &imported.project.document.project;
        if current_project.id != imported_project.id {
            let conflict = MergeConflict::DifferentProject {
                current: current_project.id.clone(),
                imported: imported_project.id.clone(),
            };
            return Self {
                project: MergeAction::Conflict(conflict.clone()),
                entities: imported
                    .entities
                    .keys()
                    .map(|id| (*id, MergeAction::Conflict(conflict.clone())))
                    .collect(),
                scenarios: imported
                    .scenarios
                    .keys()
                    .map(|id| (*id, MergeAction::Conflict(conflict.clone())))
                    .collect(),
            };
        }

        let current_revision = current_project.revision;
        let imported_revision = imported_project.revision;
        let project = action(
            merge_compare::project(&current.project.document, &imported.project.document),
            current_revision,
            imported_revision,
            None,
            true,
        );
        let entities = imported
            .entities
            .iter()
            .map(|(id, imported)| {
                let action = match current.entities.get(id) {
                    None => action(false, current_revision, imported_revision, None, false),
                    Some(current) => action(
                        merge_compare::entity(&current.document, &imported.document),
                        current_revision,
                        imported_revision,
                        Some((
                            current.document.node.revision,
                            imported.document.node.revision,
                        )),
                        true,
                    ),
                };
                (*id, action)
            })
            .collect();
        let scenarios = imported
            .scenarios
            .iter()
            .map(|(id, imported)| {
                let action = match current.scenarios.get(id) {
                    None => action(false, current_revision, imported_revision, None, false),
                    Some(current) => action(
                        merge_compare::scenario(&current.document, &imported.document),
                        current_revision,
                        imported_revision,
                        Some((
                            current.document.scenario.revision,
                            imported.document.scenario.revision,
                        )),
                        true,
                    ),
                };
                (*id, action)
            })
            .collect();
        Self {
            project,
            entities,
            scenarios,
        }
    }

    /// Reports whether any proposed project, entity, or scenario action conflicts.
    pub fn has_conflicts(&self) -> bool {
        matches!(self.project, MergeAction::Conflict(_))
            || self
                .entities
                .values()
                .any(|action| matches!(action, MergeAction::Conflict(_)))
            || self
                .scenarios
                .values()
                .any(|action| matches!(action, MergeAction::Conflict(_)))
    }
}

fn action(
    equal: bool,
    current_base: u64,
    imported_base: u64,
    aggregate: Option<(u64, u64)>,
    exists: bool,
) -> MergeAction {
    if equal {
        MergeAction::Unchanged
    } else if current_base != imported_base {
        MergeAction::Conflict(MergeConflict::BaseRevision {
            current: current_base,
            imported: imported_base,
        })
    } else if let Some((current, imported)) =
        aggregate.filter(|(current, imported)| current != imported)
    {
        MergeAction::Conflict(MergeConflict::AggregateRevision { current, imported })
    } else if exists {
        MergeAction::Update
    } else {
        MergeAction::Create
    }
}
