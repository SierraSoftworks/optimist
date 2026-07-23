use super::{
    AnalysisError, AnalysisLimits, AnalysisRevisionKey, Edge, Node, StructuralAnalysis,
    analysis_cycles, analysis_graph::CausalGraph, analysis_tarjan,
};

impl StructuralAnalysis {
    /// Computes exact SCC and bounded elementary-cycle topology from one snapshot.
    ///
    /// Input nodes and edges are cloned into a deterministic causal graph view;
    /// changing them after this call cannot alter the returned projection. Only
    /// `contributes`, `changes`, and `blocks` participate. Tarjan SCCs are exact.
    /// Cycle enumeration is exact up to `limits`, and sets `cycles_truncated` if
    /// another cycle exists beyond the count bound.
    pub fn compute(
        revision: AnalysisRevisionKey,
        nodes: &[Node],
        edges: &[Edge],
        limits: AnalysisLimits,
    ) -> Result<Self, AnalysisError> {
        if limits.maximum_cycle_length == 0 || limits.maximum_cycles == 0 {
            return Err(AnalysisError::InvalidLimits);
        }
        let graph = CausalGraph::new(nodes, edges)?;
        let components = analysis_tarjan::components(&graph);
        let (cycles, cycles_truncated) = analysis_cycles::enumerate(&graph, limits);
        Ok(Self {
            revision,
            components,
            cycles,
            cycles_truncated,
            limits,
        })
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::domain::{
        BlockingEffect, Distribution, EdgePayload, EntityId, Estimate, EstimateId, Factor,
        NodeKind, NodePayload, ProjectId, SignedInfluence,
    };

    fn revision() -> AnalysisRevisionKey {
        AnalysisRevisionKey {
            project: ProjectId::new("analysis").unwrap(),
            graph_revision: 7,
            scenario: None,
            dependence_revision: None,
            formula_revision: 2,
        }
    }

    fn node(id: u64) -> Node {
        Node::new(
            EntityId::new(id),
            format!("node-{id}"),
            format!("Node {id}"),
            NodePayload::Factor(Factor {
                controllable: false,
                evidence: vec![],
            }),
        )
        .unwrap()
    }

    fn edge(source: u64, destination: u64) -> Edge {
        Edge::new(
            EntityId::new(source),
            NodeKind::Factor,
            EntityId::new(destination),
            NodeKind::Factor,
            EdgePayload::Blocks(BlockingEffect {
                degree: Estimate::<SignedInfluence>::new(
                    EstimateId::new(0),
                    Distribution::scaled_beta(2.0, 2.0, -1.0, 1.0).unwrap(),
                )
                .unwrap(),
            }),
        )
        .unwrap()
    }

    #[test]
    fn finds_exact_components_and_canonical_cycles() {
        let nodes = [node(0), node(1), node(2), node(3)];
        let edges = [edge(0, 1), edge(1, 0), edge(1, 2), edge(2, 2)];
        let analysis = StructuralAnalysis::compute(
            revision(),
            &nodes,
            &edges,
            AnalysisLimits::new(4, 10).unwrap(),
        )
        .unwrap();
        assert_eq!(
            analysis
                .components
                .iter()
                .map(|component| component.nodes.clone())
                .collect::<Vec<_>>(),
            vec![
                vec![EntityId::new(0), EntityId::new(1)],
                vec![EntityId::new(2)],
                vec![EntityId::new(3)],
            ]
        );
        assert!(analysis.components[0].is_feedback);
        assert!(analysis.components[1].is_feedback);
        assert!(!analysis.components[2].is_feedback);
        assert_eq!(analysis.cycles.len(), 2);
        assert_eq!(
            analysis.cycles[0].nodes,
            vec![EntityId::new(0), EntityId::new(1)]
        );
        assert_eq!(analysis.cycles[1].nodes, vec![EntityId::new(2)]);
    }

    #[test]
    fn ignores_noncausal_edges_and_reports_truncation_deterministically() {
        let nodes = [node(0), node(1)];
        let noncausal = Edge::new(
            EntityId::new(0),
            NodeKind::Factor,
            EntityId::new(1),
            NodeKind::Factor,
            EdgePayload::PartOf,
        )
        .unwrap();
        let empty = StructuralAnalysis::compute(
            revision(),
            &nodes,
            &[noncausal],
            AnalysisLimits::default(),
        )
        .unwrap();
        assert!(empty.cycles.is_empty());
        assert!(
            empty
                .components
                .iter()
                .all(|component| !component.is_feedback)
        );

        let cycles = [edge(0, 0), edge(1, 1)];
        let truncated = StructuralAnalysis::compute(
            revision(),
            &nodes,
            &cycles,
            AnalysisLimits::new(2, 1).unwrap(),
        )
        .unwrap();
        assert_eq!(truncated.cycles.len(), 1);
        assert!(truncated.cycles_truncated);
        assert_eq!(truncated.cycles[0].nodes, vec![EntityId::new(0)]);
    }

    proptest! {
        #[test]
        fn components_partition_nodes_and_cycles_are_canonical(
            size in 1_usize..8,
            candidates in proptest::collection::vec((0_usize..8, 0_usize..8), 0..32),
        ) {
            let nodes: Vec<_> = (0..size).map(|id| node(id as u64)).collect();
            let edges: Vec<_> = candidates
                .into_iter()
                .filter(|(source, destination)| *source < size && *destination < size)
                .map(|(source, destination)| edge(source as u64, destination as u64))
                .collect();
            let analysis = StructuralAnalysis::compute(
                revision(),
                &nodes,
                &edges,
                AnalysisLimits::new(size, 1_000).unwrap(),
            )
            .unwrap();
            let members: Vec<_> = analysis
                .components
                .iter()
                .flat_map(|component| component.nodes.iter().copied())
                .collect();
            let unique: std::collections::BTreeSet<_> = members.iter().copied().collect();
            prop_assert_eq!(members.len(), size);
            prop_assert_eq!(unique.len(), size);

            for cycle in &analysis.cycles {
                let cycle_nodes: std::collections::BTreeSet<_> =
                    cycle.nodes.iter().copied().collect();
                prop_assert_eq!(cycle_nodes.len(), cycle.nodes.len());
                prop_assert_eq!(cycle.nodes.iter().min(), cycle.nodes.first());
                prop_assert_eq!(cycle.edges.len(), cycle.nodes.len());
                for (index, edge) in cycle.edges.iter().enumerate() {
                    prop_assert_eq!(edge.source, cycle.nodes[index]);
                    prop_assert_eq!(
                        edge.destination,
                        cycle.nodes[(index + 1) % cycle.nodes.len()]
                    );
                }
            }
        }
    }
}
