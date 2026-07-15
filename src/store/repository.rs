use crate::domain::{Edge, EdgeId, EntityId, Node, ProjectId};

use super::RepositoryResult;

pub trait GraphRepository {
    fn project_id(&self) -> &ProjectId;

    fn next_entity_id(&self) -> RepositoryResult<EntityId>;

    fn create_node(&mut self, node: Node) -> RepositoryResult<()>;

    fn get_node(&self, id: EntityId) -> RepositoryResult<Option<Node>>;

    fn list_nodes(&self) -> RepositoryResult<Vec<Node>>;

    fn delete_node(&mut self, id: EntityId) -> RepositoryResult<Node>;

    fn create_edge(&mut self, edge: Edge) -> RepositoryResult<()>;

    fn get_edge(&self, id: &EdgeId) -> RepositoryResult<Option<Edge>>;

    fn list_edges(&self) -> RepositoryResult<Vec<Edge>>;

    fn delete_edge(&mut self, id: &EdgeId) -> RepositoryResult<Edge>;
}
