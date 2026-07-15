use std::collections::BTreeMap;

use crate::domain::{Edge, EdgeId, EntityId, Node, ProjectId};

use super::validation::name_claims;
use super::{GraphRepository, RepositoryError, RepositoryResult};

pub struct InMemoryRepository {
    project_id: ProjectId,
    next_entity_id: Option<u64>,
    nodes: BTreeMap<EntityId, Node>,
    names: BTreeMap<String, EntityId>,
    edges: BTreeMap<EdgeId, Edge>,
}

impl InMemoryRepository {
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
        if self.next_entity_id.is_some_and(|next| id.value() >= next) {
            self.next_entity_id = id.value().checked_add(1);
        }
        Ok(())
    }

    fn get_node(&self, id: EntityId) -> RepositoryResult<Option<Node>> {
        Ok(self.nodes.get(&id).cloned())
    }

    fn list_nodes(&self) -> RepositoryResult<Vec<Node>> {
        Ok(self.nodes.values().cloned().collect())
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
        let source_kind = self
            .nodes
            .get(&edge.source)
            .ok_or(RepositoryError::MissingEntity(edge.source))?
            .kind();
        let destination_kind = self
            .nodes
            .get(&edge.destination)
            .ok_or(RepositoryError::MissingEntity(edge.destination))?
            .kind();
        for (id, actual, declared) in [
            (edge.source, source_kind, edge.source_kind),
            (edge.destination, destination_kind, edge.destination_kind),
        ] {
            if actual != declared {
                return Err(RepositoryError::EndpointKindMismatch {
                    id,
                    actual,
                    declared,
                });
            }
        }

        let revision = edge.revision;
        let mut edge = Edge::new(
            edge.source,
            source_kind,
            edge.destination,
            destination_kind,
            edge.payload,
        )?;
        edge.revision = revision;
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
}
