use indradb::{BulkInsertItem, Edge as IndraEdge, Identifier, Json, Vertex};

use crate::domain::{Edge, EdgeId, Node, NodeKind};

use super::super::{RepositoryError, RepositoryResult};

pub(super) const NODE_PROPERTY: &str = "optimist_node";
pub(super) const EDGE_PROPERTY: &str = "optimist_edge";
pub(super) const NORMALIZED_NAME_PROPERTY: &str = "optimist_name";

pub(super) fn identifier(value: &str) -> RepositoryResult<Identifier> {
    Identifier::new(value).map_err(|error| RepositoryError::Datastore(error.to_string()))
}

pub(super) fn node_items(node: &Node) -> RepositoryResult<Vec<BulkInsertItem>> {
    let id = node.id.to_indradb_uuid();
    let vertex = Vertex::with_id(id, identifier(node_kind(node.kind()))?);
    let payload = serde_json::to_value(node)
        .map(Json::new)
        .map_err(|error| RepositoryError::InvalidPayload(error.to_string()))?;
    let normalized_name = Json::new(serde_json::Value::String(node.normalized_name.clone()));

    Ok(vec![
        BulkInsertItem::Vertex(vertex),
        BulkInsertItem::VertexProperty(id, identifier(NODE_PROPERTY)?, payload),
        BulkInsertItem::VertexProperty(id, identifier(NORMALIZED_NAME_PROPERTY)?, normalized_name),
    ])
}

pub(super) fn edge_items(edge: &Edge) -> RepositoryResult<Vec<BulkInsertItem>> {
    let key = edge_key(&edge.id())?;
    let payload = serde_json::to_value(edge)
        .map(Json::new)
        .map_err(|error| RepositoryError::InvalidPayload(error.to_string()))?;
    Ok(vec![
        BulkInsertItem::Edge(key.clone()),
        BulkInsertItem::EdgeProperty(key, identifier(EDGE_PROPERTY)?, payload),
    ])
}

pub(super) fn edge_key(id: &EdgeId) -> RepositoryResult<IndraEdge> {
    Ok(IndraEdge::new(
        id.source.to_indradb_uuid(),
        identifier(id.kind.token())?,
        id.destination.to_indradb_uuid(),
    ))
}

fn node_kind(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Outcome => "outcome",
        NodeKind::Metric => "metric",
        NodeKind::Factor => "factor",
        NodeKind::Intervention => "intervention",
    }
}
