use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    domain::{
        Edge, EntityId, MonteCarloConfig, Node, NodePayload, ProjectDependenceModel,
        ScalarPreference, Scenario, ScenarioBudget, ScenarioDraft, ScenarioId, ScenarioObjective,
    },
    project::Project,
};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProjectHeader {
    pub(super) schema_version: u32,
    pub(super) project: Project,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) dependence: Option<ProjectDependenceModel>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EntityHeader {
    pub(super) schema_version: u32,
    pub(super) base_project_revision: u64,
    pub(super) node: NodeHeader,
    #[serde(default)]
    pub(super) outgoing_edges: Vec<Edge>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ScenarioDocumentHeader {
    pub(super) schema_version: u32,
    pub(super) base_project_revision: u64,
    pub(super) scenario: ScenarioHeader,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ScenarioHeader {
    pub(super) id: ScenarioId,
    pub(super) revision: u64,
    pub(super) name: String,
    pub(super) title: String,
    pub(super) objectives: Vec<ScenarioObjective>,
    pub(super) planning_horizon: u64,
    #[serde(default)]
    pub(super) budgets: Vec<ScenarioBudget>,
    #[serde(default)]
    pub(super) candidate_interventions: Vec<EntityId>,
    pub(super) monte_carlo: MonteCarloConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) scalar_preferences: Option<Vec<ScalarPreference>>,
}

impl ScenarioHeader {
    pub(super) fn from_scenario(scenario: &Scenario) -> Self {
        Self {
            id: scenario.id,
            revision: scenario.revision,
            name: scenario.draft.name.clone(),
            title: scenario.draft.title.clone(),
            objectives: scenario.draft.objectives.clone(),
            planning_horizon: scenario.draft.planning_horizon,
            budgets: scenario.draft.budgets.clone(),
            candidate_interventions: scenario.draft.candidate_interventions.clone(),
            monte_carlo: scenario.draft.monte_carlo,
            scalar_preferences: scenario.draft.scalar_preferences.clone(),
        }
    }

    pub(super) fn into_scenario(self, rationale: String) -> Scenario {
        Scenario {
            id: self.id,
            revision: self.revision,
            draft: ScenarioDraft {
                name: self.name,
                title: self.title,
                rationale,
                objectives: self.objectives,
                planning_horizon: self.planning_horizon,
                budgets: self.budgets,
                candidate_interventions: self.candidate_interventions,
                monte_carlo: self.monte_carlo,
                scalar_preferences: self.scalar_preferences,
            },
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NodeHeader {
    pub(super) id: EntityId,
    pub(super) revision: u64,
    pub(super) name: String,
    pub(super) normalized_name: String,
    pub(super) title: String,
    #[serde(default)]
    pub(super) aliases: Vec<String>,
    #[serde(default)]
    pub(super) metadata: BTreeMap<String, serde_json::Value>,
    pub(super) payload: NodePayload,
}

impl NodeHeader {
    pub(super) fn from_node(node: &Node) -> Self {
        Self {
            id: node.id,
            revision: node.revision,
            name: node.name.clone(),
            normalized_name: node.normalized_name.clone(),
            title: node.title.clone(),
            aliases: node.aliases.clone(),
            metadata: node.metadata.clone(),
            payload: node.payload.clone(),
        }
    }

    pub(super) fn into_node(self, description: String) -> Node {
        Node {
            id: self.id,
            revision: self.revision,
            name: self.name,
            normalized_name: self.normalized_name,
            title: self.title,
            description,
            aliases: self.aliases,
            metadata: self.metadata,
            payload: self.payload,
        }
    }
}
