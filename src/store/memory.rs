use std::collections::BTreeMap;

use crate::domain::{Edge, EdgeId, EntityId, Node, ProjectId};

use super::memory_edges;
use super::validation::{advance_entity_id, name_claims};
use super::{GraphRepository, RepositoryError, RepositoryResult};

/// Deterministic in-process repository used for fast domain and command tests.
///
/// This implementation enforces the same semantic contract as IndraDB without
/// persistence. Each instance represents exactly one project.
pub struct InMemoryRepository {
    project_id: ProjectId,
    next_entity_id: Option<u64>,
    nodes: BTreeMap<EntityId, Node>,
    names: BTreeMap<String, EntityId>,
    edges: BTreeMap<EdgeId, Edge>,
}

impl InMemoryRepository {
    /// Creates an empty repository scoped to `project_id`.
    ///
    /// ```
    /// use optimist::{domain::ProjectId, store::InMemoryRepository};
    /// let repository = InMemoryRepository::new(ProjectId::new("delivery")?);
    /// # Ok::<(), optimist::domain::IdError>(())
    /// ```
    pub fn new(project_id: ProjectId) -> Self {
        Self {
            project_id,
            next_entity_id: Some(0),
            nodes: BTreeMap::new(),
            names: BTreeMap::new(),
            edges: BTreeMap::new(),
        }
    }
}

impl GraphRepository for InMemoryRepository {
    fn project_id(&self) -> &ProjectId {
        &self.project_id
    }

    fn next_entity_id(&self) -> RepositoryResult<EntityId> {
        self.next_entity_id
            .map(EntityId::new)
            .ok_or(RepositoryError::IdentifierSpaceExhausted)
    }

    fn create_node(&mut self, node: Node) -> RepositoryResult<()> {
        if self.nodes.contains_key(&node.id) {
            return Err(RepositoryError::DuplicateEntity(node.id));
        }
        let claims = name_claims(&node)?;
        if let Some(claim) = claims.iter().find(|claim| self.names.contains_key(*claim)) {
            return Err(RepositoryError::DuplicateName(claim.clone()));
        }

        let id = node.id;
        for claim in claims {
            self.names.insert(claim, id);
        }
        self.nodes.insert(id, node);
        advance_entity_id(&mut self.next_entity_id, id);
        Ok(())
    }

    fn get_node(&self, id: EntityId) -> RepositoryResult<Option<Node>> {
        Ok(self.nodes.get(&id).cloned())
    }

    fn list_nodes(&self) -> RepositoryResult<Vec<Node>> {
        Ok(self.nodes.values().cloned().collect())
    }

    fn update_node(&mut self, node: Node) -> RepositoryResult<()> {
        let current = self
            .nodes
            .get(&node.id)
            .ok_or(RepositoryError::MissingEntity(node.id))?;
        super::validation::validate_node_update(current, &node)?;
        self.nodes.insert(node.id, node);
        Ok(())
    }

    fn delete_node(&mut self, id: EntityId) -> RepositoryResult<Node> {
        if self
            .edges
            .values()
            .any(|edge| edge.source == id || edge.destination == id)
        {
            return Err(RepositoryError::EntityHasEdges(id));
        }
        let node = self
            .nodes
            .remove(&id)
            .ok_or(RepositoryError::MissingEntity(id))?;
        for claim in name_claims(&node)? {
            self.names.remove(&claim);
        }
        Ok(node)
    }

    fn create_edge(&mut self, edge: Edge) -> RepositoryResult<()> {
        let edge = memory_edges::validated(&self.nodes, edge)?;
        let id = edge.id();
        if self.edges.contains_key(&id) {
            return Err(RepositoryError::DuplicateEdge(id.to_string()));
        }
        self.edges.insert(id, edge);
        Ok(())
    }

    fn get_edge(&self, id: &EdgeId) -> RepositoryResult<Option<Edge>> {
        Ok(self.edges.get(id).cloned())
    }

    fn list_edges(&self) -> RepositoryResult<Vec<Edge>> {
        Ok(self.edges.values().cloned().collect())
    }

    fn update_edge(&mut self, edge: Edge) -> RepositoryResult<()> {
        memory_edges::update(&self.nodes, &mut self.edges, edge)
    }

    fn delete_edge(&mut self, id: &EdgeId) -> RepositoryResult<Edge> {
        self.edges
            .remove(id)
            .ok_or_else(|| RepositoryError::MissingEdge(id.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        Edge, EdgePayload, EntityId, Factor, Measurement, MeasurementPolarity, Metric, Node,
        NodeKind, NodePayload, ProjectId,
    };

    use super::{GraphRepository, InMemoryRepository, RepositoryError};

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
        .expect("valid factor")
    }

    fn metric(id: u64, name: &str) -> Node {
        Node::new(
            EntityId::new(id),
            name,
            name,
            NodePayload::Metric(Metric {
                unit: "ratio".to_owned(),
                aggregation: None,
            }),
        )
        .expect("valid metric")
    }

    fn repository(name: &str) -> InMemoryRepository {
        InMemoryRepository::new(ProjectId::new(name).expect("valid project ID"))
    }

    #[test]
    fn projects_have_independent_identifier_and_name_scopes() {
        let mut first = repository("first");
        let mut second = repository("second");
        first.create_node(factor(0, "Reliability")).unwrap();
        second.create_node(factor(0, "Reliability")).unwrap();
        assert_eq!(first.next_entity_id().unwrap(), EntityId::new(1));
        assert_eq!(second.next_entity_id().unwrap(), EntityId::new(1));
    }

    #[test]
    fn update_node_replaces_only_payload_and_revision() {
        let mut repository = repository("update_node");
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
    fn normalized_names_and_aliases_are_unique() {
        let mut repository = repository("names");
        let mut node = factor(0, "Delivery Reliability");
        node.aliases.push("Deploy Health".to_owned());
        repository.create_node(node).unwrap();

        let error = repository
            .create_node(factor(1, "  DEPLOY health "))
            .unwrap_err();
        assert_eq!(
            error,
            RepositoryError::DuplicateName("deploy health".to_owned())
        );
    }

    #[test]
    fn edges_require_existing_endpoints_with_matching_kinds() {
        let mut repository = repository("edges");
        repository.create_node(metric(0, "Availability")).unwrap();
        repository.create_node(factor(1, "Capacity")).unwrap();
        let edge = Edge::new(
            EntityId::new(0),
            NodeKind::Metric,
            EntityId::new(1),
            NodeKind::Factor,
            EdgePayload::Measures(Measurement {
                polarity: MeasurementPolarity::HigherIsBetter,
                observations: Vec::new(),
            }),
        )
        .unwrap();
        let id = edge.id();
        repository.create_edge(edge.clone()).unwrap();
        assert_eq!(repository.get_edge(&id).unwrap(), Some(edge));
        assert!(matches!(
            repository.create_edge(repository.get_edge(&id).unwrap().unwrap()),
            Err(RepositoryError::DuplicateEdge(_))
        ));
    }

    #[test]
    fn nodes_with_edges_cannot_be_deleted() {
        let mut repository = repository("deletion");
        repository.create_node(metric(0, "Availability")).unwrap();
        repository.create_node(factor(1, "Capacity")).unwrap();
        repository
            .create_edge(
                Edge::new(
                    EntityId::new(0),
                    NodeKind::Metric,
                    EntityId::new(1),
                    NodeKind::Factor,
                    EdgePayload::Measures(Measurement {
                        polarity: MeasurementPolarity::HigherIsBetter,
                        observations: Vec::new(),
                    }),
                )
                .unwrap(),
            )
            .unwrap();

        assert_eq!(
            repository.delete_node(EntityId::new(1)),
            Err(RepositoryError::EntityHasEdges(EntityId::new(1)))
        );
    }

    #[test]
    fn missing_edges_have_a_distinct_error() {
        let mut repository = repository("missing_edge");
        let edge = Edge::new(
            EntityId::new(0),
            NodeKind::Metric,
            EntityId::new(1),
            NodeKind::Factor,
            EdgePayload::Measures(Measurement {
                polarity: MeasurementPolarity::HigherIsBetter,
                observations: Vec::new(),
            }),
        )
        .unwrap();

        assert!(matches!(
            repository.delete_edge(&edge.id()),
            Err(RepositoryError::MissingEdge(_))
        ));
    }

    #[test]
    fn updates_existing_edge_payloads() {
        let mut repository = repository("update_edge");
        repository.create_node(metric(0, "Availability")).unwrap();
        repository.create_node(factor(1, "Capacity")).unwrap();
        let mut edge = Edge::new(
            EntityId::new(0),
            NodeKind::Metric,
            EntityId::new(1),
            NodeKind::Factor,
            EdgePayload::Measures(Measurement {
                polarity: MeasurementPolarity::HigherIsBetter,
                observations: Vec::new(),
            }),
        )
        .unwrap();
        repository.create_edge(edge.clone()).unwrap();
        edge.revision = 1;
        repository.update_edge(edge.clone()).unwrap();
        assert_eq!(repository.get_edge(&edge.id()).unwrap(), Some(edge));
    }
}
