use std::collections::{BTreeMap, BTreeSet};

use super::{EntityId, StronglyConnectedComponent, analysis_graph::CausalGraph};

pub(super) fn components(graph: &CausalGraph) -> Vec<StronglyConnectedComponent> {
    let mut state = State {
        graph,
        next_index: 0,
        stack: Vec::new(),
        on_stack: BTreeSet::new(),
        indices: BTreeMap::new(),
        lowlinks: BTreeMap::new(),
        components: Vec::new(),
    };
    for node in &graph.nodes {
        if !state.indices.contains_key(node) {
            state.visit(*node);
        }
    }
    state.components.sort_by_key(|component| component.nodes[0]);
    state.components
}

struct State<'a> {
    graph: &'a CausalGraph,
    next_index: usize,
    stack: Vec<EntityId>,
    on_stack: BTreeSet<EntityId>,
    indices: BTreeMap<EntityId, usize>,
    lowlinks: BTreeMap<EntityId, usize>,
    components: Vec<StronglyConnectedComponent>,
}

impl State<'_> {
    fn visit(&mut self, node: EntityId) {
        let index = self.next_index;
        self.next_index += 1;
        self.indices.insert(node, index);
        self.lowlinks.insert(node, index);
        self.stack.push(node);
        self.on_stack.insert(node);

        let successors: Vec<_> = self.graph.successors(node).collect();
        for successor in successors {
            if !self.indices.contains_key(&successor) {
                self.visit(successor);
                let lowlink = self.lowlinks[&node].min(self.lowlinks[&successor]);
                self.lowlinks.insert(node, lowlink);
            } else if self.on_stack.contains(&successor) {
                let lowlink = self.lowlinks[&node].min(self.indices[&successor]);
                self.lowlinks.insert(node, lowlink);
            }
        }
        if self.lowlinks[&node] == self.indices[&node] {
            self.finish_component(node);
        }
    }

    fn finish_component(&mut self, root: EntityId) {
        let mut nodes = Vec::new();
        loop {
            let node = self.stack.pop().expect("root remains on Tarjan stack");
            self.on_stack.remove(&node);
            nodes.push(node);
            if node == root {
                break;
            }
        }
        nodes.sort();
        let members: BTreeSet<_> = nodes.iter().copied().collect();
        let edges = self.graph.internal_edges(&members);
        let is_feedback = nodes.len() > 1 || self.graph.has_self_loop(nodes[0]);
        self.components.push(StronglyConnectedComponent {
            nodes,
            edges,
            is_feedback,
        });
    }
}
