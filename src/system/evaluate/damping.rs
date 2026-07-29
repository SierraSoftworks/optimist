//! Choosing how far each draw steps toward the value it is converging on.

use crate::system::values::Stride;

/// Smallest step the adaptive damping will tighten to.
const MINIMUM: f64 = 0.02;

/// Largest step it will open to, which is the whole way to the computed value.
const CEILING: f64 = 1.0;

/// Contracting passes that must pass before a tightened step is relaxed again.
const RECOVERY_PASSES: u32 = 8;

/// Growth over the previous pass that counts as having overshot.
const OVERSHOT: f64 = 1.05;

/// The stride each draw takes, tightened when it overshoots and opened when it
/// has been closing steadily.
///
/// A model carries a thousand draws at once and they do not all sit in the same
/// regime: most settle readily while a few, drawn near a fold, cycle at a step
/// the rest converge happily at. A single stride shared between them has to suit
/// the worst of them, which is slow — and worse than slow, because the path a
/// draw takes decides which fixed point it reaches when a design has more than
/// one. Sharing a stride therefore makes a draw's answer depend on which other
/// draws it happened to be solved beside, which is why the ensemble could not be
/// divided freely and why the configured figure could not be raised.
///
/// Giving each draw its own stride removes both. A draw near a fold tightens
/// until it stops overshooting and stays there; the draws beside it open up to
/// the full step and settle in a fraction of the passes; and a draw solved in a
/// share of four reaches the same place as the same draw solved whole.
///
/// The configured damping is where every draw opens, not a ceiling: a draw that
/// has been contracting steadily has shown the step it is taking is safe, so it
/// is allowed to lengthen beyond it.
pub(super) struct Damping {
    strides: Vec<f64>,
    previous: Vec<f64>,
    contracting: Vec<u32>,
    shared: Option<f64>,
}

impl Damping {
    pub(super) fn opening(at: f64, draws: usize) -> Self {
        let stride = at.clamp(MINIMUM, CEILING);
        Self {
            strides: vec![stride; draws],
            previous: vec![f64::INFINITY; draws],
            contracting: vec![0; draws],
            shared: Some(stride),
        }
    }

    pub(super) fn stride(&self) -> Stride<'_> {
        match self.shared {
            Some(stride) => Stride::Shared(stride),
            None => Stride::PerDraw(&self.strides),
        }
    }

    /// Adapts every draw's stride to how far that draw just moved.
    pub(super) fn adapt(&mut self, movement: &[f64]) {
        let mut shared = self.strides.first().copied();
        for (draw, moved) in movement.iter().enumerate() {
            let stride = &mut self.strides[draw];
            if *moved > self.previous[draw] * OVERSHOT {
                *stride = (*stride * 0.5).max(MINIMUM);
                self.contracting[draw] = 0;
            } else {
                self.contracting[draw] += 1;
                if self.contracting[draw] >= RECOVERY_PASSES {
                    *stride = (*stride * 2.0).min(CEILING);
                    self.contracting[draw] = 0;
                }
            }
            self.previous[draw] = *moved;
            shared = shared.filter(|first| first == stride);
        }
        self.shared = shared;
    }

    pub(super) fn retain(
        &mut self,
        before: crate::squiggle::distribution::Ensemble,
        after: crate::squiggle::distribution::Ensemble,
        size: usize,
    ) {
        self.strides = retained(&self.strides, before, after, size);
        self.previous = retained(&self.previous, before, after, size);
        self.contracting = retained(&self.contracting, before, after, size);
        self.shared = self
            .strides
            .first()
            .copied()
            .filter(|first| self.strides.iter().all(|stride| stride == first));
    }
}

fn retained<T: Copy>(
    values: &[T],
    before: crate::squiggle::distribution::Ensemble,
    after: crate::squiggle::distribution::Ensemble,
    size: usize,
) -> Vec<T> {
    let mut offset = 0;
    let mut kept = Vec::with_capacity(after.width(size));
    for (block, width) in before.live_blocks(size) {
        if after.live() >> block & 1 != 0 {
            kept.extend_from_slice(&values[offset..offset + width]);
        }
        offset += width;
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strides(damping: &Damping) -> Vec<f64> {
        damping.strides.clone()
    }

    #[test]
    fn a_draw_that_overshoots_is_tightened_alone() {
        let mut damping = Damping::opening(0.4, 3);
        damping.adapt(&[1.0, 1.0, 1.0]);
        damping.adapt(&[0.5, 0.5, 2.0]);
        assert_eq!(strides(&damping), vec![0.4, 0.4, 0.2]);
    }

    /// Until a draw is tightened away from the rest, every draw shares one
    /// stride and the blend can take the cheaper path.
    #[test]
    fn draws_stepping_together_are_reported_as_one_stride() {
        let mut damping = Damping::opening(0.4, 3);
        assert!(matches!(damping.stride(), Stride::Shared(0.4)));
        damping.adapt(&[1.0, 1.0, 1.0]);
        damping.adapt(&[0.5, 0.5, 2.0]);
        assert!(matches!(damping.stride(), Stride::PerDraw(_)));
    }

    /// A draw that has been closing steadily is allowed past the configured
    /// figure, which is where it opens rather than as far as it may go.
    #[test]
    fn a_steadily_closing_draw_opens_beyond_the_configured_step() {
        let mut damping = Damping::opening(0.2, 1);
        for pass in 0..RECOVERY_PASSES * 3 {
            damping.adapt(&[1.0 / f64::from(pass + 1)]);
        }
        assert_eq!(strides(&damping), vec![CEILING.min(0.2 * 8.0)]);
    }

    #[test]
    fn tightening_stops_at_the_minimum() {
        let mut damping = Damping::opening(0.2, 1);
        let mut moved = 1.0;
        for _ in 0..32 {
            moved *= 2.0;
            damping.adapt(&[moved]);
        }
        assert_eq!(strides(&damping), vec![MINIMUM]);
    }

    #[test]
    fn retiring_blocks_keeps_the_stride_of_each_live_draw() {
        let before = crate::squiggle::distribution::Ensemble::whole(64);
        let after = before.retaining(0x0000_0000_0000_0005);
        let mut damping = Damping::opening(0.2, 64);
        damping.strides = (0..64).map(|draw| draw as f64).collect();
        damping.retain(before, after, 64);
        assert_eq!(strides(&damping), vec![0.0, 2.0]);
    }
}
