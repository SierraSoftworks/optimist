//! Numeric access to evaluated quantities, independent of how they were produced.
//!
//! A channel may resolve to a certain number or to an uncertain sample set, and
//! the solver has to treat both the same way: blend one iterate toward the next,
//! and measure how far apart two iterates are. Both operations run per draw, so a
//! quantity is handled here as a vector of draws with a certain value being the
//! degenerate case of a vector whose entries agree.

use rand_chacha::ChaCha20Rng;

use crate::squiggle::{Distribution, Value};

/// Reads a quantity as `count` aligned draws.
///
/// A number repeats across every draw, which keeps it aligned with the uncertain
/// quantities it is combined with rather than being broadcast at some later
/// point where the correspondence would already have been lost.
pub(super) fn draws(value: &Value, count: usize, rng: &mut ChaCha20Rng) -> Option<Vec<f64>> {
    match value {
        Value::Number(number) => Some(vec![*number; count]),
        Value::Distribution(distribution) => Some(distribution.draws(count, rng).ok()?.to_vec()),
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

/// Moves `previous` a fraction of the way toward `next`.
///
/// Feedback around a loop can overshoot: a rise in utilisation lengthens the
/// queue, which raises occupancy, which raises utilisation again. Taking a
/// partial step damps that oscillation so the iteration settles instead of
/// swinging between two states forever. Damping changes only the path to the
/// fixed point, never the fixed point itself, because where the two iterates
/// agree the blend returns that shared value unchanged.
pub(super) fn blend(previous: &[f64], next: &[f64], weight: f64) -> Vec<f64> {
    previous
        .iter()
        .zip(next)
        .map(|(previous, next)| previous + weight * (next - previous))
        .collect()
}

/// Returns the largest relative gap between two iterates.
///
/// The gap is scaled by the magnitude of the values being compared, so a
/// throughput of millions and a probability near zero are held to the same
/// standard. Comparing per draw rather than by summary is what makes the test
/// meaningful: two iterates can share a mean while disagreeing about which draws
/// have saturated, and a solver that stopped there would report a settled system
/// that had not settled.
pub(super) fn distance(previous: &[f64], next: &[f64]) -> f64 {
    previous
        .iter()
        .zip(next)
        .map(|(previous, next)| {
            let scale = previous.abs().max(next.abs()).max(1.0);
            (next - previous).abs() / scale
        })
        .fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use super::*;

    fn rng() -> ChaCha20Rng {
        ChaCha20Rng::seed_from_u64(3)
    }

    #[test]
    fn a_number_repeats_across_every_draw() {
        assert_eq!(
            draws(&Value::Number(2.5), 3, &mut rng()),
            Some(vec![2.5, 2.5, 2.5])
        );
    }

    #[test]
    fn an_uncertain_quantity_reads_its_own_draws() {
        let distribution = Distribution::from_samples(vec![1.0, 2.0, 3.0]).expect("samples");
        assert_eq!(
            draws(&Value::Distribution(distribution), 3, &mut rng()),
            Some(vec![1.0, 2.0, 3.0])
        );
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
    fn blending_moves_toward_the_new_iterate() {
        assert_eq!(blend(&[0.0, 10.0], &[10.0, 0.0], 0.5), vec![5.0, 5.0]);
        assert_eq!(blend(&[3.0], &[9.0], 1.0), vec![9.0]);
    }

    #[test]
    fn blending_a_settled_value_leaves_it_alone() {
        assert_eq!(blend(&[7.0, 7.0], &[7.0, 7.0], 0.25), vec![7.0, 7.0]);
    }

    #[test]
    fn distance_is_relative_to_magnitude() {
        // A million against a million and one is closer than one against two.
        assert!(distance(&[1e6], &[1e6 + 1.0]) < distance(&[1.0], &[2.0]));
        assert_eq!(distance(&[5.0], &[5.0]), 0.0);
    }

    #[test]
    fn distance_reports_the_worst_draw_not_the_average() {
        assert_eq!(distance(&[0.0, 0.0, 0.0], &[0.0, 0.0, 1.0]), 1.0);
    }
}
