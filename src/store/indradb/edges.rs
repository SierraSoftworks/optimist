use indradb::{Database, Datastore, Json, SpecificEdgeQuery};

use crate::domain::{Edge, EdgeId};

use super::{
    codec::{EDGE_PROPERTY, edge_items, edge_key, identifier},
    queries,
};
use crate::store::{RepositoryError, RepositoryResult};

pub(super) fn create<D: Datastore>(database: &Database<D>, edge: Edge) -> RepositoryResult<()> {
    let source = queries::get_node(database, edge.source)?
        .ok_or(RepositoryError::MissingEntity(edge.source))?;
    let destination = queries::get_node(database, edge.destination)?
        .ok_or(RepositoryError::MissingEntity(edge.destination))?;
    let revision = edge.revision;
    let description = edge.description;
    let metadata = edge.metadata;
    let mut edge = Edge::new(
        edge.source,
        source.kind(),
        edge.destination,
        destination.kind(),
        edge.payload,
    )?;
    edge.revision = revision;
    edge.description = description;
    edge.metadata = metadata;
    let id = edge.id();
    if queries::get_edge(database, &id)?.is_some() {
        return Err(RepositoryError::DuplicateEdge(id.to_string()));
    }
    database
        .bulk_insert(edge_items(&edge)?)
        .map_err(queries::storage_error)
}

pub(super) fn update<D: Datastore>(database: &Database<D>, edge: Edge) -> RepositoryResult<()> {
    let id = edge.id();
    if queries::get_edge(database, &id)?.is_none() {
        return Err(RepositoryError::MissingEdge(id.to_string()));
    }
    validate_endpoint_kinds(database, &edge)?;
    let value = serde_json::to_value(&edge)
        .map(Json::new)
        .map_err(|error| RepositoryError::InvalidPayload(error.to_string()))?;
    database
        .set_properties(
            SpecificEdgeQuery::single(edge_key(&id)?),
            identifier(EDGE_PROPERTY)?,
            &value,
        )
        .map_err(queries::storage_error)
}

pub(super) fn delete<D: Datastore>(database: &Database<D>, id: &EdgeId) -> RepositoryResult<Edge> {
    let edge = queries::get_edge(database, id)?
        .ok_or_else(|| RepositoryError::MissingEdge(id.to_string()))?;
    database
        .delete(SpecificEdgeQuery::single(edge_key(id)?))
        .map_err(queries::storage_error)?;
    Ok(edge)
}

fn validate_endpoint_kinds<D: Datastore>(
    database: &Database<D>,
    edge: &Edge,
) -> RepositoryResult<()> {
    for (id, declared) in [
        (edge.source, edge.source_kind),
        (edge.destination, edge.destination_kind),
    ] {
        let actual = queries::get_node(database, id)?
            .ok_or(RepositoryError::MissingEntity(id))?
            .kind();
        if actual != declared {
            return Err(RepositoryError::EndpointKindMismatch {
                id,
                actual,
                declared,
            });
        }
    }
    Ok(())
}
