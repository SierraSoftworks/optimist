//! Dropping draws that have already reached their own fixed point.
//!
//! A relaxation carries a thousand draws and they do not finish together. On the
//! shipped examples most of them are exactly still within a third of the passes
//! the solve takes, while a handful near a fold go on moving; every pass after
//! that recomputes a whole ensemble on behalf of those few. The draws that have
//! stopped are not being refined by it, because they have stopped in the
//! strongest sense available: the pass reproduced their previous values bit for
//! bit.
//!
//! That is what makes dropping them exact rather than approximate. Draws are
//! independent, so draw `i` evolves only from draw `i`, and the model is
//! deterministic, so a pass that maps a value to itself will map it to itself
//! again forever. Skipping such a draw elides a no-op. It is not a tolerance
//! being loosened, and no reported figure changes because of it.
//!
//! Retirement is by block rather than by draw so that the live set fits in the
//! `u64` an [`Ensemble`] carries by value. Blocks are only ever dropped, never
//! restored, which is what lets the narrowed state be read back against the
//! draws it was cut from.

use std::collections::BTreeMap;

use rand_chacha::ChaCha20Rng;

use crate::{
    squiggle::{Value, distribution::Ensemble},
    system::{
        model::ComponentId,
        values::{draws, from_draws},
    },
};

use super::config::EvaluationConfig;
use super::state::{ComponentState, LinkId, LinkState};
use crate::system::values::Varying;

/// Blocks worth retiring once fewer than this share of live draws still move.
///
/// Narrowing costs a pass over the state, so retiring one block at a time as it
/// falls still would spend more on bookkeeping than it saves. Waiting until a
/// quarter of the live draws have stopped keeps that cost amortised.
const WORTHWHILE: f64 = 0.25;

/// Which of `live`'s blocks held every one of their draws still.
///
/// `motion` is the largest movement each live draw saw over the pass, in the
/// order the live blocks supply them. Returns the blocks to drop, or `None` when
/// too few have fallen still to be worth the narrowing.
pub(super) fn spent(motion: &[f64], live: Ensemble, size: usize) -> Option<u64> {
    let mut drop = 0_u64;
    let mut retired = 0_usize;
    let mut seen = 0_usize;
    for (block, width) in live.live_blocks(size) {
        let span = motion.get(seen..seen + width)?;
        if span.iter().all(|moved| *moved == 0.0) {
            drop |= 1 << block;
            retired += width;
        }
        seen += width;
    }
    (retired as f64 >= seen as f64 * WORTHWHILE).then_some(drop)
}

/// Folds how far each draw's carried backlog moved into `motion`.
///
/// A wire's backlog is the one quantity a pass carries forward rather than
/// recomputing, so a draw whose channels agreed with themselves may still be
/// draining a queue. Retiring it on the strength of its channels alone would
/// freeze it part-way, so the backlog is asked as well. The remaining link
/// figures follow from it and from the channels, and are already covered.
pub(super) fn stirred(
    before: &BTreeMap<LinkId, LinkState>,
    after: &BTreeMap<LinkId, LinkState>,
    config: EvaluationConfig,
    rng: &mut ChaCha20Rng,
    motion: &mut [f64],
) {
    for (id, after) in after {
        let Some(before) = before.get(id) else {
            motion.fill(f64::INFINITY);
            return;
        };
        let (Some(before), Some(after)) = (
            Varying::of(&before.backlog, config.ensemble(), rng),
            Varying::of(&after.backlog, config.ensemble(), rng),
        ) else {
            continue;
        };
        let span = [before.width(), after.width()]
            .into_iter()
            .flatten()
            .fold(motion.len(), usize::min);
        let (compared, beyond) = motion.split_at_mut(span);
        for (index, slot) in compared.iter_mut().enumerate() {
            *slot = slot.max(crate::system::values::gap(
                before.at(index),
                after.at(index),
            ));
        }
        beyond.fill(f64::INFINITY);
    }
}

/// Writes a narrowed component state back over the draws it was cut from.
pub(super) fn widen(
    narrowed: &BTreeMap<ComponentId, ComponentState>,
    whole: &BTreeMap<ComponentId, ComponentState>,
    live: Ensemble,
    size: usize,
    rng: &mut ChaCha20Rng,
) -> BTreeMap<ComponentId, ComponentState> {
    let mut widened = whole.clone();
    for (id, after) in narrowed {
        let Some(before) = whole.get(id) else {
            widened.insert(id.clone(), after.clone());
            continue;
        };
        let state = ComponentState {
            channels: fields(&after.channels, &before.channels, live, size, rng),
            requests: nested(&after.requests, &before.requests, live, size, rng),
            responses: nested(&after.responses, &before.responses, live, size, rng),
            arriving: nested(&after.arriving, &before.arriving, live, size, rng),
            returning: nested(&after.returning, &before.returning, live, size, rng),
        };
        widened.insert(id.clone(), state);
    }
    widened
}

/// Writes narrowed link state back over the draws it was cut from.
pub(super) fn widen_links(
    narrowed: &BTreeMap<LinkId, LinkState>,
    whole: &BTreeMap<LinkId, LinkState>,
    live: Ensemble,
    size: usize,
    rng: &mut ChaCha20Rng,
) -> BTreeMap<LinkId, LinkState> {
    let mut widened = whole.clone();
    for (id, after) in narrowed {
        let Some(before) = whole.get(id) else {
            widened.insert(id.clone(), after.clone());
            continue;
        };
        let mut restore = |after: &Value, before: &Value| scatter(after, before, live, size, rng);
        let state = LinkState {
            backlog: restore(&after.backlog, &before.backlog),
            wait: restore(&after.wait, &before.wait),
            transit: restore(&after.transit, &before.transit),
            blocked: restore(&after.blocked, &before.blocked),
            offered: restore(&after.offered, &before.offered),
            drain: restore(&after.drain, &before.drain),
            transfer: restore(&after.transfer, &before.transfer),
            bandwidth: restore(&after.bandwidth, &before.bandwidth),
        };
        widened.insert(id.clone(), state);
    }
    widened
}

fn nested(
    narrowed: &BTreeMap<String, BTreeMap<String, Value>>,
    whole: &BTreeMap<String, BTreeMap<String, Value>>,
    live: Ensemble,
    size: usize,
    rng: &mut ChaCha20Rng,
) -> BTreeMap<String, BTreeMap<String, Value>> {
    let mut widened = whole.clone();
    for (port, after) in narrowed {
        let signals = whole.get(port).map_or_else(
            || after.clone(),
            |before| fields(after, before, live, size, rng),
        );
        widened.insert(port.clone(), signals);
    }
    widened
}

fn fields(
    narrowed: &BTreeMap<String, Value>,
    whole: &BTreeMap<String, Value>,
    live: Ensemble,
    size: usize,
    rng: &mut ChaCha20Rng,
) -> BTreeMap<String, Value> {
    let mut widened = whole.clone();
    for (name, after) in narrowed {
        let value = whole.get(name).map_or_else(
            || after.clone(),
            |before| scatter(after, before, live, size, rng),
        );
        widened.insert(name.clone(), value);
    }
    widened
}

/// Places a narrowed quantity's draws back at the positions they came from.
///
/// The retired draws keep whatever they last held, which is the value they
/// reached and stopped at. Where the narrowed quantity is not a sample set of
/// the expected width it is taken as it stands, because there is then nothing to
/// interleave.
fn scatter(
    narrowed: &Value,
    whole: &Value,
    live: Ensemble,
    size: usize,
    rng: &mut ChaCha20Rng,
) -> Value {
    let width = live.retaining(u64::MAX).width(size);
    let kept = live.width(size);
    let (Some(mut full), Some(part)) = (
        draws(whole, width, rng),
        draws(narrowed, kept, rng).filter(|part| part.len() == kept),
    ) else {
        return narrowed.clone();
    };
    for (slot, draw) in live.positions(size).zip(part) {
        full[slot] = draw;
    }
    from_draws(full).unwrap_or_else(|| narrowed.clone())
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use crate::squiggle::Distribution;

    use super::*;

    fn rng() -> ChaCha20Rng {
        ChaCha20Rng::seed_from_u64(1)
    }

    fn sampled(draws: &[f64]) -> Value {
        Value::Distribution(Distribution::from_samples(draws.to_vec()).expect("samples"))
    }

    #[test]
    fn a_block_whose_draws_all_held_still_is_retired() {
        let live = Ensemble::whole(64);
        let mut motion = vec![1.0; 64];
        for slot in motion.iter_mut().take(32) {
            *slot = 0.0;
        }
        assert_eq!(spent(&motion, live, 64), Some(0x0000_0000_FFFF_FFFF));
    }

    #[test]
    fn a_block_with_one_draw_still_moving_stays() {
        let live = Ensemble::whole(64);
        let mut motion = vec![0.0; 64];
        motion[3] = 1e-18;
        let dropped = spent(&motion, live, 64).expect("worth narrowing");
        assert_eq!(dropped >> 3 & 1, 0, "block three still moves");
        assert_eq!(dropped.count_ones(), 63);
    }

    #[test]
    fn too_little_stillness_is_not_worth_narrowing() {
        let live = Ensemble::whole(64);
        let mut motion = vec![1.0; 64];
        motion[0] = 0.0;
        assert_eq!(spent(&motion, live, 64), None);
    }

    /// Retired draws keep the values they stopped at, and live draws take the
    /// values the narrowed pass computed.
    #[test]
    fn widening_interleaves_the_live_draws_with_the_retired_ones() {
        let size = 64;
        let live = Ensemble::whole(size).retaining(0x0000_0000_0000_000F);
        let whole = sampled(&(0..size).map(|draw| draw as f64).collect::<Vec<_>>());
        let narrowed = sampled(&[100.0, 200.0, 300.0, 400.0]);
        let widened = scatter(&narrowed, &whole, live, size, &mut rng());
        let found = draws(&widened, size, &mut rng()).expect("draws");
        assert_eq!(&found[..4], [100.0, 200.0, 300.0, 400.0]);
        assert_eq!(&found[4..8], [4.0, 5.0, 6.0, 7.0]);
    }
}
