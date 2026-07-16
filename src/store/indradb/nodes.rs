use indradb::{Database, Datastore, Json, SpecificVertexQuery};

use crate::domain::Node;

use super::{
    codec::{NODE_PROPERTY, identifier},
    queries,
};
use crate::store::validation::validate_node_metadata_update;
use crate::store::{RepositoryError, RepositoryResult, validation::validate_node_update};

pub(super) fn update<D: Datastore>(database: &Database<D>, node: Node) -> RepositoryResult<()> {
    let current =
        queries::get_node(database, node.id)?.ok_or(RepositoryError::MissingEntity(node.id))?;
    validate_node_update(&current, &node)?;
    let value = serde_json::to_value(&node)
        .map(Json::new)
        .map_err(|error| RepositoryError::InvalidPayload(error.to_string()))?;
    database
        .set_properties(
            SpecificVertexQuery::single(node.id.to_indradb_uuid()),
            identifier(NODE_PROPERTY)?,
            &value,
        )
        .map_err(queries::storage_error)
}

pub(super) fn update_metadata<D: Datastore>(
    database: &Database<D>,
    node: Node,
) -> RepositoryResult<()> {
    let current =
        queries::get_node(database, node.id)?.ok_or(RepositoryError::MissingEntity(node.id))?;
    validate_node_metadata_update(&current, &node)?;
    set_payload(database, node)
}

fn set_payload<D: Datastore>(database: &Database<D>, node: Node) -> RepositoryResult<()> {
    let value = serde_json::to_value(&node)
        .map(Json::new)
        .map_err(|error| RepositoryError::InvalidPayload(error.to_string()))?;
    database
        .set_properties(
            SpecificVertexQuery::single(node.id.to_indradb_uuid()),
            identifier(NODE_PROPERTY)?,
            &value,
        )
        .map_err(queries::storage_error)
}
