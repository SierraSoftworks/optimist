//! Whether the ensemble has stopped moving where its individual draws have not.
//!
//! A relaxation is declared settled when no draw moves. That is the right test
//! for a design with one fixed point and the wrong one for a design with
//! several: past a fold, a draw can sit on a branch whose slope is steeper than
//! the damped step can follow, and it then swaps between two values forever. The
//! *ensemble* it belongs to is perfectly still — the same draws land on the same
//! two values every pass, only trading places — but the per-draw test sees a
//! quantity moving by most of its own magnitude and reports a design with no
//! steady state.
//!
//! The distinction is recovered by comparing order statistics rather than draws.
//! Sorting removes the assignment of values to draws and leaves the empirical
//! distribution, which is invariant under any permutation of the branches. So a
//! stationary mixture reads as no movement at all, whatever the length of the
//! cycle its draws are going round, while a design genuinely still converging —
//! or genuinely diverging — moves its quantiles along with its draws.
//!
//! What this cannot do is tell a stationary mixture from a cycle of the
//! *ensemble* whose period divides the window it is measured over. The window is
//! long and unrelated to anything in a model, which makes that a remote
//! coincidence rather than an impossibility.

use std::collections::BTreeMap;

use rand_chacha::ChaCha20Rng;

use crate::{
    squiggle::Value,
    system::{
        model::ComponentId,
        values::{Varying, gap},
    },
};

use super::{config::EvaluationConfig, state::ComponentState};

/// Largest movement of any channel's empirical quantile function between two
/// iterates.
///
/// Infinite where the two states do not describe the same quantities, which
/// cannot be read as agreement.
pub(super) fn drift(
    earlier: &BTreeMap<ComponentId, ComponentState>,
    later: &BTreeMap<ComponentId, ComponentState>,
    config: EvaluationConfig,
    rng: &mut ChaCha20Rng,
) -> f64 {
    let mut worst = 0.0_f64;
    for (id, before) in earlier {
        let Some(after) = later.get(id) else {
            return f64::INFINITY;
        };
        for (channel, before) in &before.channels {
            let Some(after) = after.channels.get(channel) else {
                return f64::INFINITY;
            };
            worst = worst.max(apart(before, after, config, rng));
            if !worst.is_finite() {
                return worst;
            }
        }
    }
    worst
}

/// How far one channel's distribution moved between two iterates.
fn apart(before: &Value, after: &Value, config: EvaluationConfig, rng: &mut ChaCha20Rng) -> f64 {
    let (Some(before), Some(after)) = (
        Varying::of(before, config.ensemble(), rng),
        Varying::of(after, config.ensemble(), rng),
    ) else {
        return f64::INFINITY;
    };
    match (before.spread(), after.spread()) {
        (None, None) => gap(before.at(0), after.at(0)),
        // A channel that was certain and is now spread, or the other way round,
        // has changed shape rather than position and there is nothing to align.
        (Some(_), None) | (None, Some(_)) => f64::INFINITY,
        (Some(before), Some(after)) => {
            let span = before.len().min(after.len());
            let (before, after) = (ranked(&before[..span]), ranked(&after[..span]));
            before
                .into_iter()
                .zip(after)
                .fold(0.0_f64, |worst, (before, after)| {
                    worst.max(gap(before, after))
                })
        }
    }
}

/// The draws in ascending order: the empirical quantile function, sampled at
/// every order statistic.
fn ranked(draws: &[f64]) -> Vec<f64> {
    let mut sorted = draws.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand_chacha::ChaCha20Rng;

    use crate::{
        squiggle::{Distribution, Value},
        system::{evaluate::config::EvaluationConfig, model::ComponentId},
    };

    use super::{ComponentState, drift};
    use std::collections::BTreeMap;

    fn state(channel: &str, draws: Vec<f64>) -> BTreeMap<ComponentId, ComponentState> {
        let mut component = ComponentState::default();
        component.channels.insert(
            channel.to_owned(),
            Value::Distribution(Distribution::from_samples(draws).expect("samples")),
        );
        BTreeMap::from([(ComponentId::new("api"), component)])
    }

    fn between(
        earlier: BTreeMap<ComponentId, ComponentState>,
        later: BTreeMap<ComponentId, ComponentState>,
    ) -> f64 {
        let config = EvaluationConfig {
            sample_count: 4,
            ..EvaluationConfig::default()
        };
        let mut rng = ChaCha20Rng::seed_from_u64(0);
        drift(&earlier, &later, config, &mut rng)
    }

    #[test]
    fn draws_that_only_traded_places_have_not_moved() {
        let moved = between(
            state("utilisation", vec![0.1, 0.9, 0.1, 0.9]),
            state("utilisation", vec![0.9, 0.1, 0.9, 0.1]),
        );
        assert_eq!(moved, 0.0);
    }

    #[test]
    fn a_distribution_that_shifted_has_moved() {
        let moved = between(
            state("utilisation", vec![0.1, 0.9, 0.1, 0.9]),
            state("utilisation", vec![0.9, 0.9, 0.9, 0.1]),
        );
        assert!(moved > 0.5, "expected a large drift, got {moved}");
    }

    #[test]
    fn a_channel_that_appeared_cannot_be_read_as_agreement() {
        let mut later = state("utilisation", vec![0.1, 0.9, 0.1, 0.9]);
        let earlier = state("utilisation", vec![0.1, 0.9, 0.1, 0.9]);
        later.remove(&ComponentId::new("api"));
        assert!(between(earlier, later).is_infinite());
    }
}
