use crate::domain::{Edge, EdgeId, EntityId, Node, ProjectId};

use super::RepositoryResult;

/// Persistence contract for one isolated project's causal graph.
///
/// Implementations must preserve project-local ID/name uniqueness, canonical edge
/// identity, endpoint integrity, and deterministic listing. Callers should serialize
/// mutations per project until optimistic command execution is layered above this API.
///
/// ```
/// use optimist::{
///     domain::ProjectId,
///     store::{GraphRepository, InMemoryRepository},
/// };
///
/// let repository = InMemoryRepository::new(ProjectId::new("delivery")?);
/// assert_eq!(repository.project_id().as_str(), "delivery");
/// # Ok::<(), optimist::domain::IdError>(())
/// ```
pub trait GraphRepository {
    /// Returns the project namespace owned by this repository instance.
    fn project_id(&self) -> &ProjectId;

    /// Peeks at the next available project-local entity ID without reserving it.
    ///
    /// A serialized command executor must create the corresponding node before
    /// another allocation attempt.
    fn next_entity_id(&self) -> RepositoryResult<EntityId>;

    /// Validates and inserts a node, atomically claiming its ID, name, and aliases.
    fn create_node(&mut self, node: Node) -> RepositoryResult<()>;

    /// Retrieves a complete node aggregate, including embedded estimates/evidence.
    fn get_node(&self, id: EntityId) -> RepositoryResult<Option<Node>>;

    /// Lists complete node aggregates in deterministic entity-ID order.
    fn list_nodes(&self) -> RepositoryResult<Vec<Node>>;

    /// Removes and returns a node only when no incident edges would dangle.
    fn delete_node(&mut self, id: EntityId) -> RepositoryResult<Node>;

    /// Validates endpoints and inserts one canonical structural relationship.
    fn create_edge(&mut self, edge: Edge) -> RepositoryResult<()>;

    /// Retrieves a complete edge aggregate by canonical tuple identity.
    fn get_edge(&self, id: &EdgeId) -> RepositoryResult<Option<Edge>>;

    /// Lists complete edge aggregates in deterministic canonical-key order.
    fn list_edges(&self) -> RepositoryResult<Vec<Edge>>;

    /// Removes and returns an edge, leaving both endpoint nodes intact.
    fn delete_edge(&mut self, id: &EdgeId) -> RepositoryResult<Edge>;
}
