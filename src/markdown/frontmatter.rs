use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    domain::{Edge, EntityId, Node, NodePayload},
    project::Project,
};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProjectHeader {
    pub(super) schema_version: u32,
    pub(super) project: Project,
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
