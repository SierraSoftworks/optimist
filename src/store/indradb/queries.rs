use indradb::{
    AllEdgeQuery, AllVertexQuery, Database, Datastore, EdgeProperties, QueryExt, SpecificEdgeQuery,
    SpecificVertexQuery, VertexProperties,
    util::{extract_edge_properties, extract_vertex_properties},
};

use crate::domain::{Edge, EdgeId, EntityId, Node};

use super::super::{RepositoryError, RepositoryResult};
use super::codec::{EDGE_PROPERTY, NODE_PROPERTY, edge_key, identifier};

pub(super) fn get_node<D: Datastore>(
    database: &Database<D>,
    id: EntityId,
) -> RepositoryResult<Option<Node>> {
    let query = SpecificVertexQuery::single(id.to_indradb_uuid())
        .properties()
        .map_err(storage_error)?
        .name(identifier(NODE_PROPERTY)?);
    decode_nodes(vertex_properties(database, query)?).map(|mut nodes| nodes.pop())
}

pub(super) fn list_nodes<D: Datastore>(database: &Database<D>) -> RepositoryResult<Vec<Node>> {
    let query = AllVertexQuery
        .properties()
        .map_err(storage_error)?
        .name(identifier(NODE_PROPERTY)?);
    let mut nodes = decode_nodes(vertex_properties(database, query)?)?;
    nodes.sort_by_key(|node| node.id);
    Ok(nodes)
}

pub(super) fn get_edge<D: Datastore>(
    database: &Database<D>,
    id: &EdgeId,
) -> RepositoryResult<Option<Edge>> {
    let query = SpecificEdgeQuery::single(edge_key(id)?)
        .properties()
        .map_err(storage_error)?
        .name(identifier(EDGE_PROPERTY)?);
    decode_edges(edge_properties(database, query)?).map(|mut edges| edges.pop())
}

pub(super) fn list_edges<D: Datastore>(database: &Database<D>) -> RepositoryResult<Vec<Edge>> {
    let query = AllEdgeQuery
        .properties()
        .map_err(storage_error)?
        .name(identifier(EDGE_PROPERTY)?);
    let mut edges = decode_edges(edge_properties(database, query)?)?;
    edges.sort_by_key(Edge::id);
    Ok(edges)
}

fn vertex_properties<D: Datastore, Q: Into<indradb::Query>>(
    database: &Database<D>,
    query: Q,
) -> RepositoryResult<Vec<VertexProperties>> {
    let output = database.get(query).map_err(storage_error)?;
    extract_vertex_properties(output)
        .ok_or_else(|| RepositoryError::InvalidPayload("expected vertex properties".to_owned()))
}

fn edge_properties<D: Datastore, Q: Into<indradb::Query>>(
    database: &Database<D>,
    query: Q,
) -> RepositoryResult<Vec<EdgeProperties>> {
    let output = database.get(query).map_err(storage_error)?;
    extract_edge_properties(output)
        .ok_or_else(|| RepositoryError::InvalidPayload("expected edge properties".to_owned()))
}

fn decode_nodes(properties: Vec<VertexProperties>) -> RepositoryResult<Vec<Node>> {
    properties
        .into_iter()
        .flat_map(|entry| entry.props)
        .map(|property| decode(&property.value))
        .collect()
}

fn decode_edges(properties: Vec<EdgeProperties>) -> RepositoryResult<Vec<Edge>> {
    properties
        .into_iter()
        .flat_map(|entry| entry.props)
        .map(|property| decode(&property.value))
        .collect()
}

fn decode<T: serde::de::DeserializeOwned>(value: &indradb::Json) -> RepositoryResult<T> {
    serde_json::from_value((**value).clone())
        .map_err(|error| RepositoryError::InvalidPayload(error.to_string()))
}

pub(super) fn storage_error(error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::Datastore(error.to_string())
}
