use std::collections::BTreeMap;

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};

use super::{
    AnalysisLimits, Edge, EntityId, Node, analysis_cycles, analysis_graph::CausalGraph,
    scenario_analysis_coupling::Coupling, scenario_analysis_edges::PropagationEdge,
    scenario_analysis_state::StateNode,
};

/// Draws used to estimate how often a loop fails to contract.
///
/// The estimate is a proportion, whose standard error is at most
/// $1/(2\sqrt{n})$, so this resolves it to about one percentage point — finer
/// than the judgement it informs, and cheap enough to run for every circuit.
const INSTABILITY_DRAWS: u64 = 2048;

/// Sampled share above which a loop is worth reviewing even if its mean settles.
const REVIEW_SHARE: f64 = 0.05;

/// Circuits weighed per scenario.
///
/// Structural analysis enumerates up to a thousand, which is the right budget
/// for a topology report. Weighing each one costs draws, and a model with more
/// than this many loops has a structural problem to read there rather than a
/// stability estimate to read here.
const MAX_WEIGHED_CIRCUITS: usize = 64;

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
    /// Share of sampled draws in which this loop fails to contract.
    ///
    /// The gain above is one number derived from the responses' means, and a
    /// mean says nothing about how often a product crosses one. A loop of two
    /// responses averaging 0.9 apiece has a mean gain of 0.81 and looks safe,
    /// yet if each is uncertain enough their product exceeds one in a large
    /// minority of draws, and those are the draws in which the projection
    /// reports its clamp rather than the plan. `None` wherever the gain is.
    pub instability: Option<f64>,
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
    /// [`Self::needs_review`] when deciding whether a loop deserves attention,
    /// because neither an unknown gain nor a mean below one is evidence of
    /// safety.
    pub fn is_amplifying(&self) -> bool {
        self.gain.is_some_and(|gain| gain.abs() >= 1.0)
    }

    /// Reports whether this loop is worth an author's attention.
    ///
    /// Three things disqualify a loop from being ignored: a mean gain that does
    /// not contract, a gain that cannot be computed at all, and a mean that
    /// contracts while the sampled product crosses one often enough to matter.
    /// The last is the case a point estimate hides, which is why the sampled
    /// share is carried alongside the mean rather than replacing it.
    pub fn needs_review(&self) -> bool {
        !self.settles() || self.instability.is_some_and(|share| share >= REVIEW_SHARE)
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
    coupling: &Coupling,
    seed: u64,
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
    let limits = AnalysisLimits::new(
        AnalysisLimits::default().maximum_cycle_length,
        MAX_WEIGHED_CIRCUITS,
    )
    .unwrap_or_default();
    let (cycles, _) = analysis_cycles::enumerate(&graph, limits);
    cycles
        .into_iter()
        .enumerate()
        .map(|(position, cycle)| {
            let circuit = circuit(&cycle.nodes, &indices, states, propagation);
            FeedbackLoop {
                gain: circuit.as_ref().map(|hops| gain(hops, propagation)),
                instability: circuit.as_ref().map(|hops| {
                    // Each circuit gets its own stream so adding a loop to the
                    // model cannot change the estimate reported for another.
                    instability(
                        hops,
                        propagation,
                        coupling,
                        seed.wrapping_add(position as u64),
                    )
                }),
                states: cycle.nodes,
            }
        })
        .collect()
}

/// Chooses one relationship per hop, or reports that the circuit cannot be weighed.
///
/// Parallel relationships compose additively in deviation, but taking the
/// largest keeps the reported gain a bound on what one trip can do rather than
/// an average that could hide an amplifying path behind a damping one.
fn circuit(
    cycle: &[EntityId],
    indices: &BTreeMap<EntityId, usize>,
    states: &[StateNode],
    propagation: &[PropagationEdge],
) -> Option<Vec<usize>> {
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
        .map(|(source, destination)| {
            propagation
                .iter()
                .enumerate()
                .filter(|(_, edge)| edge.source == *source && edge.destination == *destination)
                .max_by(|(_, left), (_, right)| {
                    left.effect
                        .marginal_mean()
                        .abs()
                        .total_cmp(&right.effect.marginal_mean().abs())
                })
                .map(|(index, _)| index)
        })
        .collect()
}

fn gain(hops: &[usize], propagation: &[PropagationEdge]) -> f64 {
    hops.iter()
        .map(|hop| propagation[*hop].effect.marginal_mean())
        .product()
}

/// Estimates how often one trip around the circuit fails to contract.
///
/// Responses are drawn jointly through the project's copula, so two hops that
/// share a quantity move together rather than being multiplied as if
/// independent — the difference matters most here, because a shared assumption
/// appearing twice on a circuit squares its effect on the gain. A draw that
/// produces a non-finite product carries no information about stability and is
/// left out of the denominator rather than counted either way.
fn instability(
    hops: &[usize],
    propagation: &[PropagationEdge],
    coupling: &Coupling,
    seed: u64,
) -> f64 {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let mut unstable = 0_u64;
    let mut valid = 0_u64;
    for _ in 0..INSTABILITY_DRAWS {
        let drawn = coupling.draw(&mut rng);
        let product: f64 = hops
            .iter()
            .map(|hop| propagation[*hop].effect.sample(&mut rng, &drawn))
            .product();
        if !product.is_finite() {
            continue;
        }
        valid += 1;
        if product.abs() >= 1.0 {
            unstable += 1;
        }
    }
    if valid == 0 {
        return 0.0;
    }
    unstable as f64 / valid as f64
}
