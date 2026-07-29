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
        Value::Distribution(distribution) => Some(
            distribution
                .draws(Ensemble::whole(count), rng)
                .ok()?
                .to_vec(),
        ),
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
pub(super) enum Varying<'a> {
    /// One value, holding for every draw.
    Uniform(f64),
    /// One value per draw, borrowed from whatever owns them.
    PerDraw(&'a [f64]),
}

impl<'a> Varying<'a> {
    /// Reads a quantity, sampling the whole ensemble if it has not been drawn from
    /// yet and keeping this share of the result.
    pub(super) fn of(
        value: &'a Value,
        ensemble: Ensemble,
        rng: &mut ChaCha20Rng,
    ) -> Option<Self> {
        match value {
            Value::Number(number) => Some(Self::Uniform(*number)),
            Value::Distribution(distribution) => {
                Some(Self::PerDraw(distribution.draws(ensemble, rng).ok()?))
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

    fn width(&self) -> Option<usize> {
        match self {
            Self::Uniform(_) => None,
            Self::PerDraw(draws) => Some(draws.len()),
        }
    }

    /// Borrows the draws, where this quantity has any of its own.
    pub(super) fn spread(&self) -> Option<&'a [f64]> {
        match self {
            Self::Uniform(_) => None,
            Self::PerDraw(draws) => Some(draws),
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

/// Moves `settled` a fraction of the way toward `computed`, reporting how far
/// the two were apart.
///
/// Feedback around a loop can overshoot: a rise in utilisation lengthens the
/// queue, which raises occupancy, which raises utilisation again. Taking a
/// partial step damps that oscillation so the iteration settles instead of
/// swinging between two states forever. Damping changes only the path to the
/// fixed point, never the fixed point itself, because where the two iterates
/// agree the blend returns that shared value unchanged.
///
/// The gap is scaled by the magnitude of the values being compared, so a
/// throughput of millions and a probability near zero are held to the same
/// standard. Comparing per draw rather than by summary is what makes the test
/// meaningful: two iterates can share a mean while disagreeing about which draws
/// have saturated, and a solver that stopped there would report a settled system
/// that had not settled.
///
/// Both come out of one pass because the solver never wants one without the
/// other, and the pass is the expensive part.
pub(super) fn converge(
    settled: &Varying,
    computed: &Varying,
    weight: f64,
    count: usize,
) -> (Value, f64) {
    if let (Varying::Uniform(settled), Varying::Uniform(computed)) = (settled, computed) {
        return (
            Value::Number(settled + weight * (computed - settled)),
            gap(*settled, *computed),
        );
    }
    let span = span(settled, computed, count);
    count!(Draws, span);
    let mut moved: f64 = 0.0;
    let mut blended = Vec::with_capacity(span);
    for index in 0..span {
        let settled = settled.at(index);
        let computed = computed.at(index);
        moved = moved.max(gap(settled, computed));
        blended.push(settled + weight * (computed - settled));
    }
    (
        from_draws(blended).unwrap_or(Value::Number(0.0)),
        moved,
    )
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
        let distribution = Distribution::from_samples(vec![1.0, 2.0, 3.0]).expect("samples");
        let value = Value::Distribution(distribution);
        let Some(Varying::PerDraw(draws)) = Varying::of(&value, Ensemble::whole(3), &mut rng())
        else {
            panic!("an uncertain quantity carries draws");
        };
        assert_eq!(draws, [1.0, 2.0, 3.0]);
    }

    fn blend(previous: &Varying, next: &Varying, weight: f64, count: usize) -> Option<Value> {
        Some(converge(previous, next, weight, count).0)
    }

    fn distance(previous: &Varying, next: &Varying, count: usize) -> f64 {
        converge(previous, next, 1.0, count).1
    }

    #[test]
    fn blending_moves_toward_the_new_iterate() {
        assert_eq!(
            blend(&Varying::Uniform(0.0), &Varying::Uniform(10.0), 0.5, 4),
            Some(Value::Number(5.0))
        );
        assert_eq!(
            blend(
                &Varying::PerDraw(&[0.0, 10.0]),
                &Varying::PerDraw(&[10.0, 0.0]),
                0.5,
                4
            ),
            Some(Value::Number(5.0))
        );
        assert_eq!(
            blend(&Varying::PerDraw(&[3.0]), &Varying::PerDraw(&[9.0]), 1.0, 4),
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
        let Some(Value::Distribution(blended)) = blend(
            &Varying::Uniform(0.0),
            &Varying::PerDraw(&[10.0, 20.0]),
            0.5,
            4,
        ) else {
            panic!("draws on one side make the result uncertain");
        };
        assert_eq!(blended.samples(), Some([5.0, 10.0].as_slice()));
    }

    /// A shorter authored sample set bounds how many draws are aligned.
    #[test]
    fn blending_stops_at_the_shorter_sample_set() {
        let Some(Value::Distribution(blended)) = blend(
            &Varying::PerDraw(&[0.0, 0.0, 0.0]),
            &Varying::PerDraw(&[10.0, 20.0]),
            1.0,
            3,
        ) else {
            panic!("disagreeing draws stay uncertain");
        };
        assert_eq!(blended.samples(), Some([10.0, 20.0].as_slice()));
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
        assert_eq!(
            distance(
                &Varying::PerDraw(&[0.0, 0.0, 0.0]),
                &Varying::PerDraw(&[0.0, 0.0, 1.0]),
                3
            ),
            1.0
        );
    }

    /// The certain and uncertain paths have to agree, since which one a quantity
    /// takes is an accident of whether its draws happened to collapse.
    #[test]
    fn a_certain_pair_measures_the_same_as_the_draws_it_stands_for() {
        let expanded = distance(
            &Varying::PerDraw(&[3.0, 3.0]),
            &Varying::PerDraw(&[4.0, 4.0]),
            2,
        );
        assert_eq!(
            distance(&Varying::Uniform(3.0), &Varying::Uniform(4.0), 2),
            expanded
        );
    }
}
