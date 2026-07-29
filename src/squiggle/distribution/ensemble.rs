//! Which draws a computation carries, and which of them it is responsible for.

use std::ops::Range;

/// A share of one sample set's draws.
///
/// # Why a share and not just a count
///
/// Every draw index carries an independent deterministic system, so a solve could
/// in principle be split across threads by giving each of them some of the draws.
/// It cannot be split by giving each of them a *smaller sample set*, because
/// sampling is defined over the whole of one:
/// [`Distribution`](crate::squiggle::Distribution) lays its strata across the unit
/// interval and shuffles them, so a quarter of a thousand stratified draws is not
/// the same set of numbers as two hundred and fifty stratified draws. A worker
/// that sampled its own share would be solving a different model.
///
/// So the two things a count was being asked to mean are separated. The *size* is
/// how many draws the model carries, and it decides what every distribution
/// samples — identically on every thread. The *window* is which of those draws
/// this worker computes. Sample the whole; keep your share.
///
/// # Why the window is a fraction rather than a range
///
/// An authored sample set fixes its own length, and combining one with a symbolic
/// quantity aligns everything to the shorter of them. A window recorded as
/// absolute indices would fall outside an array that turned out shorter than the
/// configured size. Recorded as "the second of four" it lands correctly on any
/// length, which is what lets authored data and partitioning coexist.
///
/// ```
/// use optimist::squiggle::distribution::Ensemble;
///
/// let draws: Vec<f64> = (0..10).map(f64::from).collect();
/// let shares: Vec<&[f64]> = Ensemble::split(10, 3)
///     .map(|share| share.window(&draws))
///     .collect();
///
/// assert_eq!(shares.concat(), draws);
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ensemble {
    size: usize,
    part: usize,
    parts: usize,
}

impl Ensemble {
    /// Every draw of an ensemble of `size`.
    pub const fn whole(size: usize) -> Self {
        Self {
            size,
            part: 0,
            parts: 1,
        }
    }

    /// Divides an ensemble into `parts` shares, in draw order.
    ///
    /// Shares are as even as the size allows and together cover every draw
    /// exactly once, so results computed separately concatenate back into the
    /// answer the whole ensemble would have given.
    pub fn split(size: usize, parts: usize) -> impl Iterator<Item = Self> {
        let parts = parts.max(1);
        (0..parts).map(move |part| Self { size, part, parts })
    }

    /// How many draws the whole ensemble carries.
    ///
    /// This is what a distribution samples, on every thread, whichever share is
    /// being computed.
    pub const fn size(self) -> usize {
        self.size
    }

    /// Whether this share is the whole ensemble.
    pub const fn is_whole(self) -> bool {
        self.parts == 1
    }

    /// An ensemble of the same shape at a different size.
    ///
    /// Used where operands align to an authored sample set shorter than the
    /// configured size: the share stays the same share, of fewer draws.
    pub const fn resized(self, size: usize) -> Self {
        Self { size, ..self }
    }

    /// How many draws this share carries.
    ///
    /// The share of the ensemble's own size, which is what a formula over aligned
    /// operands runs across.
    pub fn len(self) -> usize {
        self.width(self.size)
    }

    /// Whether this share carries no draws at all.
    ///
    /// Only possible when an ensemble is split into more shares than it has draws.
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// Which draw of the whole ensemble this share's `index` refers to.
    pub fn at(self, index: usize) -> usize {
        self.bounds(self.size).start + index
    }

    /// Where this share falls within `length` draws.
    ///
    /// Bounds are derived from the length rather than stored, so one share reads
    /// correctly from arrays of different lengths. Multiplying before dividing
    /// keeps the shares adjacent and exhaustive without tracking a remainder.
    fn bounds(self, length: usize) -> Range<usize> {
        let start = length * self.part / self.parts;
        let end = length * (self.part + 1) / self.parts;
        start..end
    }

    /// This share's view of a full set of draws.
    pub fn window(self, draws: &[f64]) -> &[f64] {
        if self.is_whole() {
            return draws;
        }
        &draws[self.bounds(draws.len())]
    }

    /// How many draws this share carries out of `length`.
    pub fn width(self, length: usize) -> usize {
        self.bounds(length).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shares(size: usize, parts: usize, draws: &[f64]) -> Vec<Vec<f64>> {
        Ensemble::split(size, parts)
            .map(|share| share.window(draws).to_vec())
            .collect()
    }

    #[test]
    fn the_whole_ensemble_is_every_draw() {
        let draws = [1.0, 2.0, 3.0];
        assert_eq!(Ensemble::whole(3).window(&draws), draws);
        assert!(Ensemble::whole(3).is_whole());
    }

    /// The property the whole design rests on: splitting loses and repeats nothing.
    #[test]
    fn shares_cover_every_draw_exactly_once() {
        let draws: Vec<f64> = (0..17).map(f64::from).collect();
        for parts in 1..=20 {
            assert_eq!(shares(17, parts, &draws).concat(), draws, "into {parts}");
        }
    }

    /// An uneven split differs by at most one draw, so no worker is left idle.
    #[test]
    fn shares_are_as_even_as_the_count_allows() {
        let draws: Vec<f64> = (0..10).map(f64::from).collect();
        let widths: Vec<usize> = shares(10, 4, &draws)
            .iter()
            .map(|share| share.len())
            .collect();
        assert_eq!(widths, [2, 3, 2, 3]);
    }

    /// A share reads the same fraction of an authored set as of a configured one.
    #[test]
    fn a_share_lands_on_an_array_shorter_than_the_ensemble() {
        let authored: Vec<f64> = (0..4).map(f64::from).collect();
        assert_eq!(shares(1_000, 2, &authored).concat(), authored);
    }

    /// More workers than draws leaves some with nothing rather than overlapping.
    #[test]
    fn splitting_further_than_there_are_draws_still_covers_them_once() {
        let draws = [1.0, 2.0];
        let shares = shares(2, 5, &draws);
        assert_eq!(shares.concat(), draws);
        assert_eq!(shares.iter().filter(|share| share.is_empty()).count(), 3);
    }

    #[test]
    fn width_agrees_with_the_window_it_describes() {
        let draws: Vec<f64> = (0..13).map(f64::from).collect();
        for share in Ensemble::split(13, 4) {
            assert_eq!(share.width(draws.len()), share.window(&draws).len());
        }
    }
}
