//! Numeric access to evaluated quantities, independent of how they were produced.
//!
//! A channel may resolve to a certain number or to an uncertain sample set, and
//! the solver has to treat both the same way: blend one iterate toward the next,
//! and measure how far apart two iterates are. Both operations run per draw, so a
//! quantity is handled here as a vector of draws with a certain value being the
//! degenerate case of a vector whose entries agree.

use rand_chacha::ChaCha20Rng;

use crate::{
    profile::count,
    squiggle::{Distribution, Value, distribution::Ensemble},
};

/// Reads a quantity as `count` aligned draws.
///
/// A number repeats across every draw, which keeps it aligned with the uncertain
/// quantities it is combined with rather than being broadcast at some later
/// point where the correspondence would already have been lost.
pub(super) fn draws(value: &Value, count: usize, rng: &mut ChaCha20Rng) -> Option<Vec<f64>> {
    count!(Draws, count);
    match value {
        Value::Number(number) => Some(vec![*number; count]),
        Value::Distribution(distribution) => {
            let ensemble = Ensemble::whole(count);
            Some(distribution.materialise(distribution.stream(rng), ensemble))
        }
        _ => None,
    }
}

/// Rebuilds a quantity from draws, collapsing to a number when they agree.
pub(super) fn from_draws(draws: Vec<f64>) -> Option<Value> {
    let first = *draws.first()?;
    if draws.iter().all(|draw| *draw == first) {
        return Some(Value::Number(first));
    }
    Distribution::from_samples(draws)
        .ok()
        .map(Value::Distribution)
}

/// A quantity as arithmetic sees it: one value across every draw, or one per draw.
///
/// A certain quantity is the degenerate case of an uncertain one, so the solver
/// has to blend and compare both the same way. Holding the certain case as a
/// single number rather than expanding it into a vector of repeats is what keeps
/// a constant channel from writing a thousand identical floats every time it is
/// compared against itself, which on a model of any size is most of what a pass
/// would otherwise spend its time doing.
pub(super) enum Varying {
    /// One value, holding for every draw.
    Uniform(f64),
    /// This quantity's share, resolved once and indexed thereafter.
    PerDraw(std::sync::Arc<[f64]>),
}

impl Varying {
    /// Reads a quantity, seeding and drawing it if it has not been drawn yet.
    pub(super) fn of(value: &Value, ensemble: Ensemble, rng: &mut ChaCha20Rng) -> Option<Self> {
        match value {
            Value::Number(number) => Some(Self::Uniform(*number)),
            Value::Distribution(distribution) => {
                let seed = distribution.stream(rng);
                let ensemble = Distribution::aligned([distribution], ensemble);
                Some(Self::PerDraw(distribution.drawn(seed, ensemble)))
            }
            _ => None,
        }
    }

    pub(super) fn at(&self, index: usize) -> f64 {
        match self {
            Self::Uniform(value) => *value,
            Self::PerDraw(draws) => draws[index],
        }
    }

    pub(super) fn width(&self) -> Option<usize> {
        match self {
            Self::Uniform(_) => None,
            Self::PerDraw(draws) => Some(draws.len()),
        }
    }

    /// Gathers the draws, where this quantity has any of its own.
    pub(super) fn spread(&self) -> Option<Vec<f64>> {
        match self {
            Self::Uniform(_) => None,
            Self::PerDraw(draws) => Some(draws.to_vec()),
        }
    }
}

/// How many draws several quantities have in common.
///
/// An authored sample set may be shorter than the configured draw count, and
/// combining it with a longer one has only as many aligned draws as the shorter
/// carries. Quantities that are certain place no bound at all.
pub(super) fn aligned(columns: &[Varying], count: usize) -> usize {
    columns
        .iter()
        .filter_map(Varying::width)
        .fold(count, usize::min)
}

/// Whether every quantity holds one value across all of its draws.
///
/// When they all do, a formula over them has one answer rather than a sample set,
/// and the whole per-draw loop can be skipped.
pub(super) fn all_uniform(columns: &[Varying]) -> bool {
    columns
        .iter()
        .all(|column| matches!(column, Varying::Uniform(_)))
}

/// Applies a formula to each aligned draw, collapsing a constant result.
///
/// The caller is expected to have taken the certain path first where one exists;
/// this is the branch that genuinely has to write a sample per draw.
pub(super) fn per_draw(span: usize, compute: impl FnMut(usize) -> f64) -> Option<Value> {
    count!(Draws, span);
    from_draws((0..span).map(compute).collect())
}

/// Applies a formula to two quantities, draw by draw.
///
/// Two certain quantities have a certain answer, which is worth taking as a
/// special case rather than expanding both into vectors of repeats to compute a
/// thousand copies of the same number.
pub(super) fn zip(
    left: &Varying,
    right: &Varying,
    count: usize,
    combine: impl Fn(f64, f64) -> f64,
) -> Option<Value> {
    if let (Varying::Uniform(left), Varying::Uniform(right)) = (left, right) {
        return Some(Value::Number(combine(*left, *right)));
    }
    let columns = [left, right];
    let span = columns
        .into_iter()
        .filter_map(|column| column.width())
        .fold(count, usize::min);
    per_draw(span, |index| combine(left.at(index), right.at(index)))
}

/// How many draws two quantities have in common.
///
/// An authored sample set may be shorter than the configured draw count, and
/// combining it with a longer one has only as many aligned draws as the shorter
/// carries.
fn span(previous: &Varying, next: &Varying, count: usize) -> usize {
    [previous.width(), next.width()]
        .into_iter()
        .flatten()
        .fold(count, usize::min)
}

/// How far along the way to its computed value each draw travels this pass.
///
/// Held as two cases because the shared one is the one a solve spends most of
/// its passes in, and it is what keeps a channel that agrees across every draw
/// from being expanded into a thousand copies of the same number merely to be
/// blended.
pub(super) enum Stride<'a> {
    /// Every draw steps the same fraction of the way.
    Shared(f64),
    /// Each draw steps its own fraction, having been damped separately.
    PerDraw(&'a [f64]),
}

impl Stride<'_> {
    fn at(&self, index: usize) -> f64 {
        match self {
            Self::Shared(weight) => *weight,
            Self::PerDraw(weights) => weights[index],
        }
    }
}

/// Moves `settled` a fraction of the way toward `computed`, reporting how far
/// the two were apart.
///
/// Feedback around a loop can overshoot: a rise in utilisation lengthens the
/// queue, which raises occupancy, which raises utilisation again. Taking a
/// partial step damps that oscillation so the iteration settles instead of
/// swinging between two states forever. Damping changes only the path a draw
/// takes and never the value it arrives at, because where the two iterates agree
/// the blend returns that shared value unchanged. Which of several fixed points
/// a draw arrives at is a matter of the path, so the stride is a draw's own.
///
/// The gap is scaled by the magnitude of the values being compared, so a
/// throughput of millions and a probability near zero are held to the same
/// standard. Comparing per draw rather than by summary is what makes the test
/// meaningful: two iterates can share a mean while disagreeing about which draws
/// have saturated, and a solver that stopped there would report a settled system
/// that had not settled.
///
/// Each draw's gap is taken into `moved` at that draw's index, and the largest
/// of them is returned. Both come out of one pass because the solver never wants
/// one without the other, and the pass is the expensive part.
pub(super) fn converge(
    settled: &Varying,
    computed: &Varying,
    stride: &Stride,
    count: usize,
    moved: &mut [f64],
) -> (Value, f64) {
    if let (Varying::Uniform(settled), Varying::Uniform(computed), Stride::Shared(weight)) =
        (settled, computed, stride)
    {
        let distance = gap(*settled, *computed);
        for slot in moved.iter_mut() {
            *slot = slot.max(distance);
        }
        return (
            Value::Number(settled + weight * (computed - settled)),
            distance,
        );
    }
    // Draws stepping at different rates pull a channel that agrees across all of
    // them apart until their strides come back together, which is the price of
    // letting one draw be damped without damping the rest.
    let span = span(settled, computed, count);
    count!(Draws, span);
    let mut furthest: f64 = 0.0;
    let mut blended = Vec::with_capacity(span);
    for index in 0..span {
        let settled = settled.at(index);
        let computed = computed.at(index);
        let distance = gap(settled, computed);
        furthest = furthest.max(distance);
        if let Some(slot) = moved.get_mut(index) {
            *slot = slot.max(distance);
        }
        blended.push(settled + stride.at(index) * (computed - settled));
    }
    (from_draws(blended).unwrap_or(Value::Number(0.0)), furthest)
}

/// How far apart two figures are, as a share of the larger of them.
///
/// Relative rather than absolute so that a throughput of millions and a
/// probability near zero are held to the same standard, and floored at one so
/// that a quantity approaching zero is not asked for ever finer agreement.
pub(super) fn gap(previous: f64, next: f64) -> f64 {
    let scale = previous.abs().max(next.abs()).max(1.0);
    (next - previous).abs() / scale
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use super::*;

    fn rng() -> ChaCha20Rng {
        ChaCha20Rng::seed_from_u64(3)
    }

    /// An authored sample set, so a test can name the draws it wants.
    fn sampled(draws: &[f64]) -> Value {
        Value::Distribution(Distribution::from_samples(draws.to_vec()).expect("samples"))
    }

    fn varying(value: &Value) -> Varying {
        Varying::of(value, Ensemble::whole(1_000), &mut rng()).expect("carries draws")
    }

    #[test]
    fn agreeing_draws_collapse_back_to_a_number() {
        assert_eq!(from_draws(vec![4.0, 4.0]), Some(Value::Number(4.0)));
        assert!(matches!(
            from_draws(vec![4.0, 5.0]),
            Some(Value::Distribution(_))
        ));
    }

    #[test]
    fn a_certain_quantity_is_read_without_being_expanded() {
        assert!(matches!(
            Varying::of(&Value::Number(2.5), Ensemble::whole(1_000), &mut rng()),
            Some(Varying::Uniform(2.5))
        ));
    }

    #[test]
    fn an_uncertain_quantity_is_read_as_its_own_draws() {
        let value = sampled(&[1.0, 2.0, 3.0]);
        assert_eq!(varying(&value).spread(), Some(vec![1.0, 2.0, 3.0]));
    }

    fn blend(previous: &Varying, next: &Varying, weight: f64, count: usize) -> Option<Value> {
        Some(
            converge(
                previous,
                next,
                &Stride::Shared(weight),
                count,
                &mut [0.0; 8],
            )
            .0,
        )
    }

    fn distance(previous: &Varying, next: &Varying, count: usize) -> f64 {
        converge(previous, next, &Stride::Shared(1.0), count, &mut [0.0; 8]).1
    }

    #[test]
    fn blending_moves_toward_the_new_iterate() {
        assert_eq!(
            blend(&Varying::Uniform(0.0), &Varying::Uniform(10.0), 0.5, 4),
            Some(Value::Number(5.0))
        );
        let (before, after) = (sampled(&[0.0, 10.0]), sampled(&[10.0, 0.0]));
        assert_eq!(
            blend(&varying(&before), &varying(&after), 0.5, 4),
            Some(Value::Number(5.0))
        );
        let (before, after) = (sampled(&[3.0]), sampled(&[9.0]));
        assert_eq!(
            blend(&varying(&before), &varying(&after), 1.0, 4),
            Some(Value::Number(9.0))
        );
    }

    #[test]
    fn blending_a_settled_value_leaves_it_alone() {
        assert_eq!(
            blend(&Varying::Uniform(7.0), &Varying::Uniform(7.0), 0.25, 4),
            Some(Value::Number(7.0))
        );
    }

    /// A certain and an uncertain iterate blend draw by draw, not by summary.
    #[test]
    fn blending_a_number_against_draws_holds_the_number_across_them() {
        let after = sampled(&[10.0, 20.0]);
        let Some(Value::Distribution(blended)) =
            blend(&Varying::Uniform(0.0), &varying(&after), 0.5, 4)
        else {
            panic!("draws on one side make the result uncertain");
        };
        assert_eq!(blended.samples(), Some([5.0, 10.0].as_slice()));
    }

    /// A shorter authored sample set bounds how many draws are aligned.
    #[test]
    fn blending_stops_at_the_shorter_sample_set() {
        let (before, after) = (sampled(&[0.0, 0.0, 0.0]), sampled(&[10.0, 20.0]));
        let Some(Value::Distribution(blended)) = blend(&varying(&before), &varying(&after), 1.0, 3)
        else {
            panic!("disagreeing draws stay uncertain");
        };
        assert_eq!(blended.samples(), Some([10.0, 20.0].as_slice()));
    }

    /// Each draw travels at its own rate, so a draw damped hard does not hold
    /// back the draws beside it.
    #[test]
    fn a_draw_blends_at_its_own_stride() {
        let (before, after) = (sampled(&[0.0, 0.0, 0.0]), sampled(&[10.0, 10.0, 10.0]));
        let strides = [1.0, 0.5, 0.1];
        let (blended, _) = converge(
            &varying(&before),
            &varying(&after),
            &Stride::PerDraw(&strides),
            3,
            &mut [0.0; 3],
        );
        let Value::Distribution(blended) = blended else {
            panic!("draws stepping at different rates disagree");
        };
        assert_eq!(blended.samples(), Some([10.0, 5.0, 1.0].as_slice()));
    }

    /// The gap is reported per draw so that the stride can be adapted per draw,
    /// and the furthest of them is what decides whether anything is still moving.
    #[test]
    fn every_draw_reports_its_own_gap() {
        let (before, after) = (sampled(&[2.0, 2.0, 2.0]), sampled(&[3.0, 6.0, 2.0]));
        let mut moved = [0.0; 3];
        let (_, furthest) = converge(
            &varying(&before),
            &varying(&after),
            &Stride::Shared(1.0),
            3,
            &mut moved,
        );
        assert_eq!(moved, [1.0 / 3.0, 2.0 / 3.0, 0.0]);
        assert_eq!(furthest, 2.0 / 3.0);
    }

    /// A pass takes the furthest any channel moved each draw, so one settled
    /// channel cannot mask another that is still moving.
    #[test]
    fn gaps_accumulate_across_the_channels_of_a_pass() {
        let mut moved = [0.0; 2];
        let still = sampled(&[5.0, 5.0]);
        converge(
            &varying(&still),
            &varying(&still),
            &Stride::Shared(1.0),
            2,
            &mut moved,
        );
        let (before, after) = (sampled(&[0.0, 0.0]), sampled(&[0.0, 4.0]));
        converge(
            &varying(&before),
            &varying(&after),
            &Stride::Shared(1.0),
            2,
            &mut moved,
        );
        assert_eq!(moved, [0.0, 1.0]);
    }

    #[test]
    fn distance_is_relative_to_magnitude() {
        // A million against a million and one is closer than one against two.
        assert!(
            distance(&Varying::Uniform(1e6), &Varying::Uniform(1e6 + 1.0), 1)
                < distance(&Varying::Uniform(1.0), &Varying::Uniform(2.0), 1)
        );
        assert_eq!(
            distance(&Varying::Uniform(5.0), &Varying::Uniform(5.0), 1),
            0.0
        );
    }

    #[test]
    fn distance_reports_the_worst_draw_not_the_average() {
        let (before, after) = (sampled(&[0.0, 0.0, 0.0]), sampled(&[0.0, 0.0, 1.0]));
        assert_eq!(distance(&varying(&before), &varying(&after), 3), 1.0);
    }

    /// The certain and uncertain paths have to agree, since which one a quantity
    /// takes is an accident of whether its draws happened to collapse.
    #[test]
    fn a_certain_pair_measures_the_same_as_the_draws_it_stands_for() {
        let (before, after) = (sampled(&[3.0, 3.0]), sampled(&[4.0, 4.0]));
        let expanded = distance(&varying(&before), &varying(&after), 2);
        assert_eq!(
            distance(&Varying::Uniform(3.0), &Varying::Uniform(4.0), 2),
            expanded
        );
    }
}
