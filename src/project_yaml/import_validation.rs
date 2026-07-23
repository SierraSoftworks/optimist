use std::collections::{BTreeMap, BTreeSet};

use crate::domain::{EntityId, ScenarioId, normalize_name};

use super::{EntityDocument, ImportError, ProjectDocument, ScenarioDocument, import_references};

/// A parsed YAML value paired with its project-relative source path.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceDocument<T> {
    /// Relative path used for diagnostics and deterministic planning.
    pub path: String,
    /// Parsed, aggregate-locally validated document.
    pub document: T,
}

impl<T> SourceDocument<T> {
    /// Associates a parsed document with its source path.
    pub fn new(path: impl Into<String>, document: T) -> Self {
        Self {
            path: path.into(),
            document,
        }
    }
}

/// A complete YAML project whose identities and base revision are consistent.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedImport {
    /// Canonical project metadata document.
    pub project: SourceDocument<ProjectDocument>,
    /// Entity documents indexed by project-local node identity.
    pub entities: BTreeMap<EntityId, SourceDocument<EntityDocument>>,
    /// Scenario documents indexed by project-local scenario identity.
    pub scenarios: BTreeMap<ScenarioId, SourceDocument<ScenarioDocument>>,
}

impl ValidatedImport {
    /// Indexes one immutable project export and rejects ambiguous identities.
    pub fn new(
        project: SourceDocument<ProjectDocument>,
        entities: Vec<SourceDocument<EntityDocument>>,
        scenarios: Vec<SourceDocument<ScenarioDocument>>,
    ) -> Result<Self, ImportError> {
        let mut paths = BTreeSet::from([project.path.clone()]);
        let mut names = BTreeMap::new();
        let mut scenario_names = BTreeMap::new();
        let mut entity_map: BTreeMap<EntityId, SourceDocument<EntityDocument>> = BTreeMap::new();
        let mut scenario_map: BTreeMap<ScenarioId, SourceDocument<ScenarioDocument>> =
            BTreeMap::new();
        let revision = project.document.project.revision;

        for entity in entities {
            validate_path_and_revision(
                &mut paths,
                &entity.path,
                revision,
                entity.document.base_project_revision,
            )?;
            let id = entity.document.node.id;
            if let Some(first) = entity_map.get(&id) {
                return Err(ImportError::DuplicateNode {
                    path: entity.path,
                    first_path: first.path.clone(),
                    node: id,
                });
            }
            let name = normalize_name(&entity.document.node.name);
            for name in std::iter::once(name).chain(
                entity
                    .document
                    .node
                    .aliases
                    .iter()
                    .map(|alias| normalize_name(alias)),
            ) {
                if let Some(first_path) = names.insert(name.clone(), entity.path.clone()) {
                    return Err(ImportError::DuplicateNodeName {
                        path: entity.path,
                        first_path,
                        name,
                    });
                }
            }
            entity_map.insert(id, entity);
        }
        for scenario in scenarios {
            validate_path_and_revision(
                &mut paths,
                &scenario.path,
                revision,
                scenario.document.base_project_revision,
            )?;
            let id = scenario.document.scenario.id;
            if let Some(first) = scenario_map.get(&id) {
                return Err(ImportError::DuplicateScenario {
                    path: scenario.path,
                    first_path: first.path.clone(),
                    scenario: id,
                });
            }
            let name = normalize_name(&scenario.document.scenario.draft.name);
            if let Some(first_path) = scenario_names.insert(name.clone(), scenario.path.clone()) {
                return Err(ImportError::DuplicateScenarioName {
                    path: scenario.path,
                    first_path,
                    name,
                });
            }
            scenario_map.insert(id, scenario);
        }
        import_references::validate(&project, &entity_map, &scenario_map)?;
        Ok(Self {
            project,
            entities: entity_map,
            scenarios: scenario_map,
        })
    }
}

fn validate_path_and_revision(
    paths: &mut BTreeSet<String>,
    path: &str,
    expected: u64,
    actual: u64,
) -> Result<(), ImportError> {
    if !paths.insert(path.to_owned()) {
        return Err(ImportError::DuplicatePath {
            path: path.to_owned(),
        });
    }
    if actual != expected {
        return Err(ImportError::InconsistentBaseRevision {
            path: path.to_owned(),
            expected,
            actual,
        });
    }
    Ok(())
}
