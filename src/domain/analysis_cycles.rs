use std::collections::BTreeSet;

use super::{AnalysisLimits, EdgeId, ElementaryCycle, EntityId, analysis_graph::CausalGraph};

pub(super) fn enumerate(
    graph: &CausalGraph,
    limits: AnalysisLimits,
) -> (Vec<ElementaryCycle>, bool) {
    let mut state = State {
        graph,
        limits,
        cycles: Vec::new(),
        truncated: false,
    };
    for start in &graph.nodes {
        let mut visited = BTreeSet::from([*start]);
        state.visit(
            *start,
            *start,
            &mut visited,
            &mut Vec::new(),
            &mut vec![*start],
        );
        if state.truncated {
            break;
        }
    }
    (state.cycles, state.truncated)
}

struct State<'a> {
    graph: &'a CausalGraph,
    limits: AnalysisLimits,
    cycles: Vec<ElementaryCycle>,
    truncated: bool,
}

impl State<'_> {
    fn visit(
        &mut self,
        start: EntityId,
        current: EntityId,
        visited: &mut BTreeSet<EntityId>,
        edges: &mut Vec<EdgeId>,
        nodes: &mut Vec<EntityId>,
    ) {
        let outgoing = self
            .graph
            .outgoing
            .get(&current)
            .cloned()
            .unwrap_or_default();
        for edge in outgoing {
            if self.truncated || edge.destination < start {
                continue;
            }
            let length = edges.len() + 1;
            if edge.destination == start {
                if length <= self.limits.maximum_cycle_length {
                    self.push(nodes.clone(), edges, edge);
                }
                continue;
            }
            if length >= self.limits.maximum_cycle_length || !visited.insert(edge.destination) {
                continue;
            }
            edges.push(edge.clone());
            nodes.push(edge.destination);
            self.visit(start, edge.destination, visited, edges, nodes);
            nodes.pop();
            edges.pop();
            visited.remove(&edge.destination);
        }
    }

    fn push(&mut self, nodes: Vec<EntityId>, edges: &[EdgeId], closing: EdgeId) {
        if self.cycles.len() == self.limits.maximum_cycles {
            self.truncated = true;
            return;
        }
        let mut cycle_edges = edges.to_vec();
        cycle_edges.push(closing);
        self.cycles.push(ElementaryCycle {
            nodes,
            edges: cycle_edges,
        });
    }
}
