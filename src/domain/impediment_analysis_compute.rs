use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::{
    AnalysisError, AnalysisRevisionKey, Edge, EdgeId, EdgeKind, EdgePayload, EntityId,
    ImpedimentAnalysis, ImpedimentCandidate, Node, NodePayload, RelationshipEvidence,
};

#[derive(Clone)]
struct Path {
    nodes: Vec<EntityId>,
    edges: Vec<EdgeId>,
}

impl ImpedimentAnalysis {
    /// Computes deterministic factor-to-outcome review candidates.
    ///
    /// The algorithm runs a breadth-first search from each factor over causal edges
    /// sorted by canonical edge identity. It records one canonical shortest path to
    /// every reachable outcome. This is exact graph reachability; evidence affects
    /// only the separate evidence-priority order and never changes topology rank.
    pub fn compute(
        revision: AnalysisRevisionKey,
        nodes: &[Node],
        edges: &[Edge],
    ) -> Result<Self, AnalysisError> {
        let nodes_by_id: BTreeMap<_, _> = nodes.iter().map(|node| (node.id, node)).collect();
        let mut outgoing: BTreeMap<EntityId, Vec<&Edge>> = BTreeMap::new();
        for edge in edges.iter().filter(|edge| is_causal(edge.payload.kind())) {
            for endpoint in [edge.source, edge.destination] {
                if !nodes_by_id.contains_key(&endpoint) {
                    return Err(AnalysisError::MissingNode {
                        edge: edge.id(),
                        node: endpoint,
                    });
                }
            }
            outgoing.entry(edge.source).or_default().push(edge);
        }
        for values in outgoing.values_mut() {
            values.sort_by_key(|edge| edge.id());
        }

        let outcomes: BTreeSet<_> = nodes
            .iter()
            .filter(|node| matches!(node.payload, NodePayload::Outcome(_)))
            .map(|node| node.id)
            .collect();
        let edge_by_id: BTreeMap<_, _> = edges.iter().map(|edge| (edge.id(), edge)).collect();
        let mut candidates = Vec::new();
        for node in nodes {
            let NodePayload::Factor(factor) = &node.payload else {
                continue;
            };
            let paths = shortest_outcome_paths(node.id, &outcomes, &outgoing);
            if paths.is_empty() {
                continue;
            }
            let nearest_outcome_distance = paths
                .values()
                .map(|path| path.edges.len())
                .min()
                .unwrap_or_default();
            let path_edges: BTreeSet<_> = paths
                .values()
                .flat_map(|path| path.edges.iter().cloned())
                .collect();
            let relationship_evidence: Vec<_> = path_edges
                .iter()
                .filter_map(|id| {
                    let references = causal_evidence(edge_by_id[id]);
                    (!references.is_empty()).then(|| RelationshipEvidence {
                        edge: id.clone(),
                        references,
                    })
                })
                .collect();
            let evidenced_edges: BTreeSet<_> = relationship_evidence
                .iter()
                .map(|value| value.edge.clone())
                .collect();
            candidates.push(ImpedimentCandidate {
                factor: node.id,
                controllable: factor.controllable,
                reachable_outcomes: paths.keys().copied().collect(),
                nearest_outcome_distance,
                path_edges: path_edges.iter().cloned().collect(),
                direct_evidence: factor.evidence.clone(),
                relationship_evidence,
                unsupported_path_edges: path_edges.difference(&evidenced_edges).cloned().collect(),
            });
        }
        candidates.sort_by(|left, right| {
            right
                .reachable_outcomes
                .len()
                .cmp(&left.reachable_outcomes.len())
                .then(
                    left.nearest_outcome_distance
                        .cmp(&right.nearest_outcome_distance),
                )
                .then(left.factor.cmp(&right.factor))
        });
        let topology_order: BTreeMap<_, _> = candidates
            .iter()
            .enumerate()
            .map(|(index, candidate)| (candidate.factor, index))
            .collect();
        let mut evidence_candidates: Vec<_> = candidates.iter().collect();
        evidence_candidates.sort_by(|left, right| {
            right
                .direct_evidence
                .len()
                .cmp(&left.direct_evidence.len())
                .then(relationship_reference_count(right).cmp(&relationship_reference_count(left)))
                .then(topology_order[&left.factor].cmp(&topology_order[&right.factor]))
                .then(left.factor.cmp(&right.factor))
        });
        let evidence_priority = evidence_candidates
            .into_iter()
            .map(|candidate| candidate.factor)
            .collect();
        Ok(Self {
            revision,
            topology_candidates: candidates,
            evidence_priority,
        })
    }
}

fn shortest_outcome_paths(
    start: EntityId,
    outcomes: &BTreeSet<EntityId>,
    outgoing: &BTreeMap<EntityId, Vec<&Edge>>,
) -> BTreeMap<EntityId, Path> {
    let mut paths = BTreeMap::new();
    let mut seen = BTreeSet::from([start]);
    let mut queue = VecDeque::from([Path {
        nodes: vec![start],
        edges: vec![],
    }]);
    while let Some(path) = queue.pop_front() {
        let current = *path.nodes.last().expect("a path always has a node");
        for edge in outgoing.get(&current).into_iter().flatten() {
            if seen.contains(&edge.destination) {
                continue;
            }
            let mut next = path.clone();
            next.nodes.push(edge.destination);
            next.edges.push(edge.id());
            seen.insert(edge.destination);
            if outcomes.contains(&edge.destination) {
                paths.insert(edge.destination, next.clone());
            }
            queue.push_back(next);
        }
    }
    paths
}

fn causal_evidence(edge: &Edge) -> Vec<String> {
    match &edge.payload {
        EdgePayload::Contributes(value) | EdgePayload::Changes(value) => value.evidence.clone(),
        EdgePayload::Blocks(_) => Vec::new(),
        _ => Vec::new(),
    }
}

fn relationship_reference_count(candidate: &ImpedimentCandidate) -> usize {
    candidate
        .relationship_evidence
        .iter()
        .map(|value| value.references.len())
        .sum()
}

const fn is_causal(kind: EdgeKind) -> bool {
    matches!(
        kind,
        EdgeKind::Contributes | EdgeKind::Changes | EdgeKind::Blocks
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        CausalEffect, Distribution, Estimate, EstimateId, Factor, LinearResponse, NodeKind,
        Outcome, OutcomeDirection, ProjectId, QuantityValue, Unit,
    };

    fn revision() -> AnalysisRevisionKey {
        AnalysisRevisionKey {
            project: ProjectId::new("impediments").unwrap(),
            graph_revision: 4,
            scenario: None,
            dependence_revision: None,
            formula_revision: 0,
        }
    }

    fn factor(id: u64, evidence: usize) -> Node {
        Node::new(
            EntityId::new(id),
            format!("factor-{id}"),
            format!("Factor {id}"),
            NodePayload::Factor(Factor {
                controllable: id.is_multiple_of(2),
                evidence: (0..evidence)
                    .map(|record| super::super::Evidence {
                        id: record as u64,
                        revision: 0,
                        summary: format!("Evidence {record}"),
                        source: None,
                    })
                    .collect(),
            }),
        )
        .unwrap()
    }

    fn outcome(id: u64) -> Node {
        Node::new(
            EntityId::new(id),
            format!("outcome-{id}"),
            format!("Outcome {id}"),
            NodePayload::Outcome(Outcome {
                direction: OutcomeDirection::Maximize,
                evidence: vec![],
            }),
        )
        .unwrap()
    }

    fn edge(source: u64, destination: u64, evidence: &[&str]) -> Edge {
        Edge::new(
            EntityId::new(source),
            NodeKind::Factor,
            EntityId::new(destination),
            if destination >= 2 {
                NodeKind::Outcome
            } else {
                NodeKind::Factor
            },
            EdgePayload::Contributes(
                CausalEffect::linear(
                    LinearResponse {
                        source_change: 1.0,
                        source_unit: Unit::dimensionless(),
                        destination_change: Estimate::<QuantityValue>::new(
                            EstimateId::new(0),
                            Distribution::point(0.5).unwrap(),
                        )
                        .unwrap(),
                        destination_unit: Unit::dimensionless(),
                    },
                    None,
                    String::new(),
                    evidence.iter().map(|value| (*value).to_owned()).collect(),
                )
                .unwrap(),
            ),
        )
        .unwrap()
    }

    #[test]
    fn keeps_topology_and_evidence_priority_separate() {
        let nodes = [factor(0, 0), factor(1, 2), outcome(2), outcome(3)];
        let edges = [edge(0, 2, &[]), edge(0, 3, &[]), edge(1, 2, &["ADR-1"])];
        let result = ImpedimentAnalysis::compute(revision(), &nodes, &edges).unwrap();
        assert_eq!(
            result
                .topology_candidates
                .iter()
                .map(|candidate| candidate.factor)
                .collect::<Vec<_>>(),
            vec![EntityId::new(0), EntityId::new(1)]
        );
        assert_eq!(
            result.evidence_priority,
            vec![EntityId::new(1), EntityId::new(0)]
        );
        assert_eq!(result.topology_candidates[0].reachable_outcomes.len(), 2);
        assert_eq!(
            result.topology_candidates[0].unsupported_path_edges.len(),
            2
        );
        assert_eq!(result.topology_candidates[1].relationship_evidence.len(), 1);
    }

    #[test]
    fn excludes_factors_without_a_causal_path_to_an_outcome() {
        let nodes = [factor(0, 0), factor(1, 0), outcome(2)];
        let edges = [edge(0, 1, &[])];
        let result = ImpedimentAnalysis::compute(revision(), &nodes, &edges).unwrap();
        assert!(result.topology_candidates.is_empty());
        assert!(result.evidence_priority.is_empty());
    }
}
