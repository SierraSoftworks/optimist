use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{
    AnalysisLimits, Edge, EntityId, Node, analysis_cycles, analysis_graph::CausalGraph,
    scenario_analysis_edges::PropagationEdge, scenario_analysis_state::StateNode,
};

/// A feedback loop among projected states and the gain that decides its fate.
///
/// Both composition rules are linear in relative deviation. A multiplicative
/// state satisfies $x_i = b_i \prod_j (x_j/b_j)^{\varepsilon_{ji}}$, so in log
/// deviations $u_i = \log(x_i/b_i)$ it is $u_i = \sum_j \varepsilon_{ji} u_j$.
/// An additive state satisfies $x_i = b_i(1 + \sum_j \varepsilon_{ji}(x_j/b_j -
/// 1))$, so in relative deviations $d_i = (x_i - b_i)/b_i$ it is $d_i = \sum_j
/// \varepsilon_{ji} d_j$. Either way one trip around a circuit multiplies the
/// deviation by
///
/// $$ g = \prod_{(j \to i) \in C} \varepsilon_{ji}, $$
///
/// so $|g| < 1$ contracts and the loop settles, $|g| > 1$ expands and the loop
/// runs away until the destination's declared support clamps it, and $|g| = 1$
/// is marginal: a deviation neither decays nor grows. A negative gain alternates
/// sign each trip, which appears as oscillation rather than drift.
///
/// This is a linearization about the baseline evaluated at each response's mean.
/// It states what the loop does to a small deviation, not what any particular
/// Monte Carlo draw does to a large one. Structural cycle discovery lives in
/// [`crate::domain::StructuralAnalysis`], which deliberately makes no claim about
/// strength; the gain is the claim this type adds.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FeedbackLoop {
    /// Circuit members rotated so the smallest entity ID is first, matching structural analysis.
    pub states: Vec<EntityId>,
    /// Product of the mean responses around the circuit, or `None` where it has no meaning.
    ///
    /// A state computed from a node equation does not respond proportionally to
    /// its parents, so no elasticity describes its incoming edge and the product
    /// would be a number with no interpretation. The loop is still reported,
    /// because its existence is what the author needs to know.
    pub gain: Option<f64>,
}

impl FeedbackLoop {
    /// Reports whether a deviation entering this loop provably dies out.
    ///
    /// Only a known, contracting gain settles. A marginal loop holds a deviation
    /// forever rather than returning the model to its baseline, and an unknown
    /// gain is not evidence of safety: a loop closed through a node equation can
    /// run away just as far, it simply admits no elasticity to multiply. Both
    /// answer `false`, so a caller that warns on the negation warns about every
    /// loop it cannot rule out.
    pub fn settles(&self) -> bool {
        self.gain.is_some_and(|gain| gain.abs() < 1.0)
    }

    /// Reports whether the gain is known to reach or exceed one.
    ///
    /// This is the strict case, useful for wording a diagnostic. Prefer
    /// [`Self::settles`] when deciding whether a loop deserves attention, because
    /// an unknown gain is not a safe one.
    pub fn is_amplifying(&self) -> bool {
        self.gain.is_some_and(|gain| gain.abs() >= 1.0)
    }
}

/// Finds the feedback loops among projected states and weighs each one.
///
/// Enumeration is delegated to structural analysis so both surfaces report the
/// same circuits in the same canonical order. Only relationships this scenario
/// actually propagates are considered, because a loop closed through an edge the
/// projection never evaluates cannot move anything.
pub(super) fn feedback_loops(
    nodes: &[Node],
    edges: &[Edge],
    states: &[StateNode],
    propagation: &[PropagationEdge],
) -> Vec<FeedbackLoop> {
    let indices = states
        .iter()
        .enumerate()
        .map(|(index, state)| (state.id, index))
        .collect::<BTreeMap<_, _>>();
    let projected = edges
        .iter()
        .filter(|edge| {
            propagation.iter().any(|candidate| {
                indices.get(&edge.source) == Some(&candidate.source)
                    && indices.get(&edge.destination) == Some(&candidate.destination)
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let Ok(graph) = CausalGraph::new(nodes, &projected) else {
        return Vec::new();
    };
    let (cycles, _) = analysis_cycles::enumerate(&graph, AnalysisLimits::default());
    cycles
        .into_iter()
        .map(|cycle| FeedbackLoop {
            gain: gain(&cycle.nodes, &indices, states, propagation),
            states: cycle.nodes,
        })
        .collect()
}

fn gain(
    cycle: &[EntityId],
    indices: &BTreeMap<EntityId, usize>,
    states: &[StateNode],
    propagation: &[PropagationEdge],
) -> Option<f64> {
    let members = cycle
        .iter()
        .map(|id| indices.get(id).copied())
        .collect::<Option<Vec<_>>>()?;
    if members
        .iter()
        .any(|index| states[*index].relation.is_some())
    {
        return None;
    }
    members
        .iter()
        .zip(members.iter().cycle().skip(1))
        .map(|(source, destination)| response(propagation, *source, *destination))
        .product::<Option<f64>>()
}

/// Reads the strongest mean response between two states.
///
/// Parallel relationships compose additively in deviation, but taking the
/// largest keeps the reported gain a bound on what one trip can do rather than
/// an average that could hide an amplifying path behind a damping one.
fn response(edges: &[PropagationEdge], source: usize, destination: usize) -> Option<f64> {
    edges
        .iter()
        .filter(|edge| edge.source == source && edge.destination == destination)
        .map(|edge| edge.effect.marginal_mean())
        .max_by(|left, right| left.abs().total_cmp(&right.abs()))
}
