use std::collections::{BTreeMap, BTreeSet};

use super::{AnalysisError, Edge, EdgeId, EdgeKind, EntityId, Node};

#[derive(Clone)]
pub(super) struct CausalGraph {
    pub(super) nodes: Vec<EntityId>,
    pub(super) edges: BTreeMap<EdgeId, Edge>,
    pub(super) outgoing: BTreeMap<EntityId, Vec<EdgeId>>,
}

impl CausalGraph {
    pub(super) fn new(nodes: &[Node], edges: &[Edge]) -> Result<Self, AnalysisError> {
        let node_ids: BTreeSet<_> = nodes.iter().map(|node| node.id).collect();
        let mut causal_edges = BTreeMap::new();
        let mut outgoing: BTreeMap<EntityId, Vec<EdgeId>> = BTreeMap::new();
        for edge in edges.iter().filter(|edge| is_causal(edge.payload.kind())) {
            let id = edge.id();
            for endpoint in [edge.source, edge.destination] {
                if !node_ids.contains(&endpoint) {
                    return Err(AnalysisError::MissingNode {
                        edge: id,
                        node: endpoint,
                    });
                }
            }
            causal_edges.insert(id.clone(), edge.clone());
            outgoing.entry(edge.source).or_default().push(id);
        }
        for values in outgoing.values_mut() {
            values.sort();
        }
        Ok(Self {
            nodes: node_ids.into_iter().collect(),
            edges: causal_edges,
            outgoing,
        })
    }

    pub(super) fn successors(&self, node: EntityId) -> impl Iterator<Item = EntityId> + '_ {
        self.outgoing
            .get(&node)
            .into_iter()
            .flatten()
            .map(|id| id.destination)
    }

    pub(super) fn internal_edges(&self, nodes: &BTreeSet<EntityId>) -> Vec<EdgeId> {
        self.edges
            .keys()
            .filter(|edge| nodes.contains(&edge.source) && nodes.contains(&edge.destination))
            .cloned()
            .collect()
    }

    pub(super) fn has_self_loop(&self, node: EntityId) -> bool {
        self.outgoing
            .get(&node)
            .is_some_and(|edges| edges.iter().any(|edge| edge.destination == node))
    }
}

fn is_causal(kind: EdgeKind) -> bool {
    matches!(
        kind,
        EdgeKind::Contributes | EdgeKind::Changes | EdgeKind::Blocks
    )
}
