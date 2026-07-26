//! Weighs the responses that compound around a feedback loop.
//!
//! A circuit's gain is the product of the responses on it, so in logs it is a
//! sum, $\ln|g| = \sum_k \ln|\varepsilon_k|$. Each relationship therefore has an
//! additive share of the compounding, which is what tells an author where to
//! intervene: a response above one contributes a positive share and pushes the
//! loop toward running away, one below contributes a negative share and damps
//! it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{
    Edge, EdgePayload, EntityId, Node, NodeKind, NodePayload, QuantitySupport, RelationBindings,
    RelationProgram, state_relation_schema,
};

/// Relative step used to differentiate an equation at its baseline.
///
/// Large enough that a Squiggle program's own arithmetic noise does not dominate
/// the difference, small enough that the local slope is still local.
const STEP: f64 = 1e-3;

/// Bound on a reported log share, so a response of zero stays representable.
///
/// A hop whose response is zero breaks the loop outright, and $\ln 0$ is not a
/// number a reader or a bar chart can use. Clamping says "overwhelmingly
/// damping" without pretending to a precision the decomposition does not have.
const MAX_CONTRIBUTION: f64 = 20.0;

/// Passes used to settle equation-backed baselines before differentiating.
///
/// An equation defines its state, so its rest point is the equation evaluated at
/// its parents' rest points rather than whatever estimate was authored beside
/// it. On a circuit that is a fixed point, which is relaxed for rather than
/// solved; the bound stops an amplifying loop from iterating forever.
const SETTLING_PASSES: usize = 64;

/// One relationship's share of a loop's compounding.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LoopWeight {
    /// State the response acts from.
    pub source: EntityId,
    /// State the response acts on.
    pub destination: EntityId,
    /// Proportional response of the destination to the source at baseline.
    ///
    /// For a relationship this is the authored elasticity. For a state computed
    /// from a node equation it is the equation's local elasticity, measured by
    /// nudging this parent about its baseline, which is the same linearisation
    /// the gain itself assumes.
    pub response: f64,
    /// Additive share of $\ln|g|$, positive where the response amplifies.
    pub contribution: f64,
}

/// Reads each state's baseline, which is where responses are measured.
///
/// Loop gain is a linearisation about the model at rest, so it starts from the
/// authored current estimates rather than anything a scenario projects. States
/// carrying an equation are then settled onto it: the equation defines the
/// state, so its rest point is the equation evaluated at its parents' rest
/// points, and an authored estimate that disagrees is a modelling
/// disagreement rather than the point to linearise about. A state with no
/// estimate has no rest point at all and is simply absent, which makes every
/// circuit through it unweighable.
/// Where each state rests, both as authored and as its equations leave it.
pub(super) struct Baselines {
    /// The state's own authored estimate, which `baseline` binds to in an equation.
    authored: BTreeMap<EntityId, f64>,
    /// Where each state comes to rest once every equation has been applied.
    settled: BTreeMap<EntityId, f64>,
}

pub(super) fn baselines(nodes: &[Node], edges: &[Edge]) -> Baselines {
    let authored = nodes
        .iter()
        .filter_map(|node| {
            let estimate = match &node.payload {
                NodePayload::Metric(metric) => metric.current.as_ref(),
                _ => node.native_state.as_ref()?.current.as_ref(),
            }?;
            Some((node.id, estimate.distribution.mean()))
        })
        .collect::<BTreeMap<_, _>>();
    let mut settled = authored.clone();
    let by_id = nodes.iter().map(|node| (node.id, node)).collect();
    let Ok(mut runtime) = RelationProgram::runtime(0) else {
        return Baselines { authored, settled };
    };
    let programs = nodes
        .iter()
        .filter_map(|node| {
            let relation = state_relation_schema::relation_of(node)?;
            let program = state_relation_schema::compile(node, &by_id, edges, relation).ok()?;
            Some((node.id, program))
        })
        .collect::<Vec<_>>();
    for _ in 0..SETTLING_PASSES {
        let mut moved = false;
        for (id, program) in &programs {
            let Some(node) = by_id.get(id) else { continue };
            // `baseline` is the state's own authored estimate throughout. Feeding
            // the settled value back would make the binding self-referential and
            // let a relative equation drift on every pass.
            let anchors = Baselines {
                authored: authored.clone(),
                settled: settled.clone(),
            };
            let Some(bindings) =
                equation_bindings(node, &by_id, edges, &anchors.authored, &anchors.settled)
            else {
                continue;
            };
            let Ok(value) = program.evaluate(&mut runtime, &bindings) else {
                continue;
            };
            if !value.is_finite() {
                continue;
            }
            // Propagation clamps every period to the state's declared support,
            // so a rest point that ignored it would put the elasticity somewhere
            // the projection can never go.
            let value = clamp_to_support(node, value);
            let previous = settled.insert(*id, value);
            if previous.is_none_or(|was| (was - value).abs() > was.abs().max(1.0) * 1e-9) {
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    Baselines { authored, settled }
}

/// Measures every response on a circuit, or reports that it cannot be measured.
///
/// `None` means some hop has no usable response: a missing baseline, an equation
/// that will not evaluate, or a state whose baseline is zero and so has no ratio
/// scale to take an elasticity against.
pub(super) fn weigh(
    cycle: &[EntityId],
    nodes: &BTreeMap<EntityId, &Node>,
    edges: &[Edge],
    baselines: &Baselines,
) -> Option<Vec<LoopWeight>> {
    let mut runtime = RelationProgram::runtime(0).ok()?;
    let weights = cycle
        .iter()
        .zip(cycle.iter().cycle().skip(1))
        .map(|(source, destination)| {
            let response = response(*source, *destination, nodes, edges, baselines, &mut runtime)?;
            response.is_finite().then(|| LoopWeight {
                source: *source,
                destination: *destination,
                response,
                contribution: response
                    .abs()
                    .ln()
                    .clamp(-MAX_CONTRIBUTION, MAX_CONTRIBUTION),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(weights)
}

/// Reads one hop's proportional response, from its equation where it has one.
fn response(
    source: EntityId,
    destination: EntityId,
    nodes: &BTreeMap<EntityId, &Node>,
    edges: &[Edge],
    baselines: &Baselines,
    runtime: &mut crate::squiggle::Runtime,
) -> Option<f64> {
    let owner = nodes.get(&destination)?;
    match state_relation_schema::relation_of(owner) {
        Some(relation) => {
            let program = state_relation_schema::compile(owner, nodes, edges, relation).ok()?;
            let parent = nodes.get(&source)?.name.clone();
            equation_elasticity(&program, &parent, owner, nodes, edges, baselines, runtime)
        }
        None => authored(source, destination, edges),
    }
}

/// Reads the strongest authored elasticity between two states.
///
/// Parallel relationships compose additively in deviation, but taking the
/// largest keeps the reported share a bound on what one trip can do rather than
/// an average that could hide an amplifying path behind a damping one.
fn authored(source: EntityId, destination: EntityId, edges: &[Edge]) -> Option<f64> {
    edges
        .iter()
        .filter(|edge| edge.source == source && edge.destination == destination)
        .filter_map(|edge| match &edge.payload {
            EdgePayload::Contributes(effect) => Some(effect.response.distribution.mean()),
            EdgePayload::Blocks(effect) if edge.destination_kind != NodeKind::Intervention => {
                Some(effect.degree.distribution.mean())
            }
            _ => None,
        })
        .max_by(|left, right| left.abs().total_cmp(&right.abs()))
}

/// Differentiates an equation with respect to one parent, in relative terms.
///
/// The elasticity is $\varepsilon = \frac{\partial x_i}{\partial x_j} \cdot
/// \frac{x_j}{x_i}$, estimated by a central difference about the baseline:
///
/// $$ \varepsilon \approx \frac{f(b_j(1+h)) - f(b_j(1-h))}{2h \, f(\mathbf{b})} $$
///
/// which is the same linearisation the loop gain assumes for an authored
/// response, so the two are directly comparable and multiply together. A
/// baseline of zero has no ratio scale and yields nothing rather than an
/// infinity.
#[allow(clippy::too_many_arguments)]
fn equation_elasticity(
    program: &RelationProgram,
    parent: &str,
    owner: &Node,
    nodes: &BTreeMap<EntityId, &Node>,
    edges: &[Edge],
    baselines: &Baselines,
    runtime: &mut crate::squiggle::Runtime,
) -> Option<f64> {
    let bindings = equation_bindings(owner, nodes, edges, &baselines.authored, &baselines.settled)?;
    let centre = baselines.settled.get(&owner.id).copied()?;
    let anchor = *bindings.parents.get(parent)?;
    if anchor == 0.0 || centre == 0.0 {
        return None;
    }
    let mut nudge = |factor: f64| {
        let mut moved = bindings.clone();
        moved.parents.insert(parent.to_owned(), anchor * factor);
        program.evaluate(runtime, &moved).ok()
    };
    let high = nudge(1.0 + STEP)?;
    let low = nudge(1.0 - STEP)?;
    let elasticity = (high - low) / (2.0 * STEP * centre);
    elasticity.is_finite().then_some(elasticity)
}

/// Holds a value inside the support its state declares.
fn clamp_to_support(node: &Node, value: f64) -> f64 {
    let support = match &node.payload {
        NodePayload::Metric(metric) => metric.quantity.support,
        _ => node
            .native_state
            .as_ref()
            .map_or(QuantitySupport::Real, |state| state.quantity.support),
    };
    match support {
        QuantitySupport::Real => value,
        QuantitySupport::NonNegative => value.max(0.0),
        QuantitySupport::Bounded { lower, upper } => value.clamp(lower, upper),
    }
}

/// Binds every name an equation declares to its baseline, with nothing intervening.
fn equation_bindings(
    owner: &Node,
    nodes: &BTreeMap<EntityId, &Node>,
    edges: &[Edge],
    authored: &BTreeMap<EntityId, f64>,
    settled: &BTreeMap<EntityId, f64>,
) -> Option<RelationBindings> {
    let relation = state_relation_schema::relation_of(owner)?;
    let parents = edges
        .iter()
        .filter(|edge| {
            edge.destination == owner.id
                && matches!(
                    edge.payload,
                    EdgePayload::Contributes(_) | EdgePayload::Blocks(_)
                )
        })
        .filter_map(|edge| {
            let name = nodes.get(&edge.source)?.name.clone();
            Some((name, settled.get(&edge.source).copied()?))
        })
        .collect::<BTreeMap<_, _>>();
    let activations = edges
        .iter()
        .filter(|edge| {
            edge.destination == owner.id && matches!(edge.payload, EdgePayload::Changes(_))
        })
        .filter_map(|edge| Some((nodes.get(&edge.source)?.name.clone(), 0.0)))
        .collect();
    Some(RelationBindings {
        baseline: authored.get(&owner.id).copied()?,
        parents,
        parameters: relation
            .parameters
            .iter()
            .map(|(name, parameter)| (name.clone(), parameter.value.distribution.mean()))
            .collect(),
        activations,
    })
}
