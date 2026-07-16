use std::collections::BTreeMap;

use indradb::MemoryDatastore;
use indradb::{Database, Datastore, SpecificVertexQuery};

use crate::domain::{Edge, EdgeId, EntityId, Node, ProjectId};

use super::super::validation::{advance_entity_id, name_claims};
use super::super::{GraphRepository, RepositoryError, RepositoryResult};
use super::codec::{NORMALIZED_NAME_PROPERTY, identifier, node_items};
use super::edges;
use super::nodes;
use super::queries;

/// [`GraphRepository`] implementation backed by an embedded IndraDB datastore.
///
/// Each instance owns one project namespace. Node and edge payloads are serialized
/// with their structural item using IndraDB bulk insertion. Cross-call atomicity is
/// **not** assumed for RocksDB; higher-level multi-item commands require the planned
/// ChangeSet recovery protocol.
pub struct IndraDbRepository<D: Datastore> {
    project_id: ProjectId,
    database: Database<D>,
    next_entity_id: Option<u64>,
    names: BTreeMap<String, EntityId>,
}

impl IndraDbRepository<MemoryDatastore> {
    /// Creates an empty IndraDB memory repository for one project.
    ///
    /// ```
    /// use optimist::{domain::ProjectId, store::IndraDbRepository};
    /// let repository = IndraDbRepository::memory(ProjectId::new("delivery")?)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn memory(project_id: ProjectId) -> RepositoryResult<Self> {
        Self::from_database(project_id, MemoryDatastore::new_db())
    }
}

impl<D: Datastore> IndraDbRepository<D> {
    /// Wraps an existing database and rebuilds project-local indices from payloads.
    ///
    /// Startup fails if persisted names, aliases, or payloads violate current domain
    /// invariants. This makes corruption/schema incompatibility visible before serving
    /// requests rather than producing partial query results.
    pub fn from_database(project_id: ProjectId, database: Database<D>) -> RepositoryResult<Self> {
        database
            .index_property(identifier(NORMALIZED_NAME_PROPERTY)?)
            .map_err(queries::storage_error)?;
        let mut repository = Self {
            project_id,
            database,
            next_entity_id: Some(0),
            names: BTreeMap::new(),
        };
        for node in queries::list_nodes(&repository.database)? {
            repository.index_node(&node)?;
        }
        Ok(repository)
    }

    fn index_node(&mut self, node: &Node) -> RepositoryResult<()> {
        for claim in name_claims(node)? {
            if self.names.insert(claim.clone(), node.id).is_some() {
                return Err(RepositoryError::DuplicateName(claim));
            }
        }
        advance_entity_id(&mut self.next_entity_id, node.id);
        Ok(())
    }
}

impl<D: Datastore> GraphRepository for IndraDbRepository<D> {
    fn project_id(&self) -> &ProjectId {
        &self.project_id
    }

    fn next_entity_id(&self) -> RepositoryResult<EntityId> {
        self.next_entity_id
            .map(EntityId::new)
            .ok_or(RepositoryError::IdentifierSpaceExhausted)
    }

    fn create_node(&mut self, node: Node) -> RepositoryResult<()> {
        if self.get_node(node.id)?.is_some() {
            return Err(RepositoryError::DuplicateEntity(node.id));
        }
        let claims = name_claims(&node)?;
        if let Some(claim) = claims.iter().find(|claim| self.names.contains_key(*claim)) {
            return Err(RepositoryError::DuplicateName(claim.clone()));
        }
        self.database
            .bulk_insert(node_items(&node)?)
            .map_err(queries::storage_error)?;
        self.index_node(&node)
    }

    fn get_node(&self, id: EntityId) -> RepositoryResult<Option<Node>> {
        queries::get_node(&self.database, id)
    }

    fn list_nodes(&self) -> RepositoryResult<Vec<Node>> {
        queries::list_nodes(&self.database)
    }

    fn update_node(&mut self, node: Node) -> RepositoryResult<()> {
        nodes::update(&self.database, node)
    }

    fn update_node_metadata(&mut self, node: Node) -> RepositoryResult<()> {
        nodes::update_metadata(&self.database, node)
    }

    fn delete_node(&mut self, id: EntityId) -> RepositoryResult<Node> {
        if self
            .list_edges()?
            .iter()
            .any(|edge| edge.source == id || edge.destination == id)
        {
            return Err(RepositoryError::EntityHasEdges(id));
        }
        let node = self
            .get_node(id)?
            .ok_or(RepositoryError::MissingEntity(id))?;
        self.database
            .delete(SpecificVertexQuery::single(id.to_indradb_uuid()))
            .map_err(queries::storage_error)?;
        for claim in name_claims(&node)? {
            self.names.remove(&claim);
        }
        Ok(node)
    }

    fn create_edge(&mut self, edge: Edge) -> RepositoryResult<()> {
        edges::create(&self.database, edge)
    }

    fn get_edge(&self, id: &EdgeId) -> RepositoryResult<Option<Edge>> {
        queries::get_edge(&self.database, id)
    }

    fn list_edges(&self) -> RepositoryResult<Vec<Edge>> {
        queries::list_edges(&self.database)
    }

    fn update_edge(&mut self, edge: Edge) -> RepositoryResult<()> {
        edges::update(&self.database, edge)
    }

    fn delete_edge(&mut self, id: &EdgeId) -> RepositoryResult<Edge> {
        edges::delete(&self.database, id)
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        Edge, EdgePayload, EntityId, Factor, Node, NodeKind, NodePayload, ProjectId, Requirement,
    };
    use crate::store::{GraphRepository, IndraDbRepository, RepositoryError};

    fn factor(id: u64, name: &str) -> Node {
        Node::new(
            EntityId::new(id),
            name,
            name,
            NodePayload::Factor(Factor {
                current: None,
                desired: None,
                controllable: true,
                evidence: Vec::new(),
            }),
        )
        .unwrap()
    }

    fn repository() -> IndraDbRepository<indradb::MemoryDatastore> {
        IndraDbRepository::memory(ProjectId::new("indra").unwrap()).unwrap()
    }

    #[test]
    fn round_trips_nodes_and_edges_through_indradb() {
        let mut repository = repository();
        repository.create_node(factor(0, "GitHub Actions")).unwrap();
        repository.create_node(factor(1, "GitHub")).unwrap();
        let edge = Edge::new(
            EntityId::new(0),
            NodeKind::Factor,
            EntityId::new(1),
            NodeKind::Factor,
            EdgePayload::Requires(Requirement {
                hard: true,
                satisfaction_threshold: None,
            }),
        )
        .unwrap();
        repository.create_edge(edge.clone()).unwrap();

        assert_eq!(
            repository.get_node(EntityId::new(0)).unwrap(),
            Some(factor(0, "GitHub Actions"))
        );
        assert_eq!(repository.get_edge(&edge.id()).unwrap(), Some(edge));
    }

    #[test]
    fn updates_only_node_payload_and_revision() {
        let mut repository = repository();
        repository.create_node(factor(0, "delivery")).unwrap();
        let mut replacement = repository.get_node(EntityId::new(0)).unwrap().unwrap();
        replacement.revision = 1;
        let NodePayload::Factor(factor) = &mut replacement.payload else {
            unreachable!()
        };
        factor.controllable = false;
        repository.update_node(replacement.clone()).unwrap();
        assert_eq!(
            repository.get_node(EntityId::new(0)).unwrap(),
            Some(replacement)
        );

        let mut invalid = repository.get_node(EntityId::new(0)).unwrap().unwrap();
        invalid.title = "Changed".to_owned();
        assert_eq!(
            repository.update_node(invalid),
            Err(RepositoryError::NodeUpdateChangedMetadata(EntityId::new(0)))
        );
    }

    #[test]
    fn metadata_updates_preserve_typed_node_payloads() {
        let mut repository = repository();
        repository.create_node(factor(0, "delivery")).unwrap();
        let mut replacement = repository.get_node(EntityId::new(0)).unwrap().unwrap();
        replacement.revision = 1;
        replacement.title = "Delivery flow".to_owned();
        replacement.description = "# Flow".to_owned();
        repository
            .update_node_metadata(replacement.clone())
            .unwrap();
        assert_eq!(
            repository.get_node(EntityId::new(0)).unwrap(),
            Some(replacement)
        );

        let mut invalid = repository.get_node(EntityId::new(0)).unwrap().unwrap();
        let NodePayload::Factor(factor) = &mut invalid.payload else {
            unreachable!()
        };
        factor.controllable = false;
        assert_eq!(
            repository.update_node_metadata(invalid),
            Err(RepositoryError::NodeMetadataUpdateChangedPayload(
                EntityId::new(0)
            ))
        );
    }

    #[test]
    fn enforces_name_uniqueness() {
        let mut repository = repository();
        repository.create_node(factor(0, "Reliability")).unwrap();
        assert!(matches!(
            repository.create_node(factor(1, " RELIABILITY ")),
            Err(RepositoryError::DuplicateName(_))
        ));
    }

    #[test]
    fn lists_nodes_by_project_local_entity_id() {
        let mut repository = repository();
        for (id, name) in [(2, "Reliability"), (0, "GitHub"), (1, "Actions")] {
            repository.create_node(factor(id, name)).unwrap();
        }

        let ids = repository
            .list_nodes()
            .unwrap()
            .into_iter()
            .map(|node| node.id)
            .collect::<Vec<_>>();
        assert_eq!(ids, [EntityId::new(0), EntityId::new(1), EntityId::new(2)]);
    }

    #[test]
    fn updates_existing_edge_payloads() {
        let mut repository = repository();
        repository.create_node(factor(0, "Actions")).unwrap();
        repository.create_node(factor(1, "GitHub")).unwrap();
        let mut edge = Edge::new(
            EntityId::new(0),
            NodeKind::Factor,
            EntityId::new(1),
            NodeKind::Factor,
            EdgePayload::Requires(Requirement {
                hard: true,
                satisfaction_threshold: None,
            }),
        )
        .unwrap();
        repository.create_edge(edge.clone()).unwrap();
        edge.revision = 1;
        edge.payload = EdgePayload::Requires(Requirement {
            hard: false,
            satisfaction_threshold: Some(0.8),
        });
        repository.update_edge(edge.clone()).unwrap();
        assert_eq!(repository.get_edge(&edge.id()).unwrap(), Some(edge));
    }
}
