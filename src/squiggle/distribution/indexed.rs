//! Computing one draw from its index, without materialising the set it belongs to.
//!
//! # What the stored array is actually for
//!
//! Two things, which can be separated:
//!
//! - **Stratification** places one draw in each $1/n$ band of the unit interval,
//!   which is what buys $O(n^{-2})$ variance on the mean against $O(n^{-1})$ for
//!   independent sampling. Under a fixed-point relaxation that is not merely
//!   accuracy: sampling noise that would be harmless in one pass becomes a
//!   convergence failure when the model is re-evaluated a thousand times.
//! - **The permutation** breaks the ascending order stratification produces.
//!   Without it every quantity's draw $i$ is its own $i$-th quantile, so two
//!   independently built quantities combined at matching indices would be
//!   perfectly rank-correlated rather than independent.
//!
//! Fisher–Yates delivers the permutation but only by holding the whole array,
//! because it decides position $i$ using choices made for every other position.
//! Nothing about the *requirement* needs that. A permutation with random access
//! serves equally well, and then a draw can be computed from its index alone:
//!
//! $$x_i = F^{-1}\left(\frac{\pi(i) + \xi_{\pi(i)}}{n}\right)$$
//!
//! with $\pi$ evaluated on demand and $\xi$ read from the stream at the position
//! belonging to that stratum. Neither term needs its neighbours, so neither needs
//! to be stored.
//!
//! # The permutation
//!
//! A four-round Feistel network over the smallest power of two containing the
//! draw count, walked past any value that lands outside it. Feistel is a
//! bijection for any round function, so bijectivity is structural rather than
//! something to verify statistically; the round function only has to mix. Cycle
//! walking keeps the domain exact, and costs under two evaluations on average
//! because the padded domain is never more than twice the count.
//!
//! This is not a cryptographic requirement — nothing here is adversarial — so
//! four rounds and a cheap integer mixer are ample. What is required is that two
//! distributions seeded differently permute differently, which is what keeps
//! their draws from lining up.
//!
//! References: Michael Luby and Charles Rackoff, *How to construct pseudorandom
//! permutations from pseudorandom functions* (1988); John Black and Phillip
//! Rogaway, *Ciphers with arbitrary finite domains* (2002), for cycle walking.

// Nothing here is adversarial, so four rounds and a cheap mixer are ample.


/// Draws of one distribution, addressed by index rather than held in an array.#[derive(Clone, Debug)]
pub(super) struct Indexed {
    seed: u64,
    count: usize,
    /// Width of each Feistel half, in bits.
    half: u32,
}

impl Indexed {
    /// Prepares to draw `count` values for the distribution identified by `seed`.
    pub(super) fn new(seed: u64, count: usize) -> Self {
        let bits = count.max(2).next_power_of_two().trailing_zeros();
        Self {
            seed,
            count: count.max(1),
            half: bits.div_ceil(2),
        }
    }

    /// The probability that draw `index` inverts.
    pub(super) fn probability(&self, index: usize) -> f64 {
        let stratum = self.stratum(index % self.count);
        let offset = self.offset(stratum);
        let width = 1.0 / self.count as f64;
        ((stratum as f64 + offset) * width).clamp(f64::EPSILON, 1.0 - f64::EPSILON)
    }

    /// Which stratum this index draws from.
    ///
    /// Cycle walking: the Feistel permutes the padded domain, and a result
    /// outside the real one is permuted again until it lands inside. Every value
    /// is reached exactly once because the padded permutation is a bijection and
    /// walking simply follows its cycles.
    fn stratum(&self, index: usize) -> usize {
        let mut walked = index;
        loop {
            walked = self.feistel(walked);
            if walked < self.count {
                return walked;
            }
        }
    }

    fn feistel(&self, value: usize) -> usize {
        let mask = (1_u64 << self.half) - 1;
        let mut left = (value as u64 >> self.half) & mask;
        let mut right = value as u64 & mask;
        for round in 0..4_u64 {
            let mixed = mix(self.seed ^ (round << 32) ^ right) & mask;
            let next = left ^ mixed;
            left = right;
            right = next;
        }
        ((left << self.half) | right) as usize
    }

    /// Where within its stratum this draw falls.
    ///
    /// Derived from the seed and the stratum rather than read from a position in
    /// a stream. Seeking a ChaCha stream would answer the same question and keep
    /// the generator the rest of the crate uses, but only if the stream is held
    /// open: re-keying it per draw costs around half a microsecond, which is two
    /// orders of magnitude more than the draw is worth. The mixer below is what
    /// the Feistel already relies on, and a stratum offset asks nothing of it
    /// that it does not provide.
    fn offset(&self, stratum: usize) -> f64 {
        let drawn = mix(self.seed ^ 0xa5a5_a5a5_dead_beef ^ (stratum as u64)) >> 11;
        drawn as f64 / (1_u64 << 53) as f64
    }
}

/// SplitMix64's finaliser, used here only to stir bits.
fn mix(value: u64) -> u64 {
    let mut mixed = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    mixed ^ (mixed >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every index reaches a different stratum, and together they reach all of them.
    ///
    /// This is what a permutation means, and it is what stratification rests on:
    /// one draw per band, no band twice.
    #[test]
    fn strata_are_visited_exactly_once() {
        for count in [1_usize, 2, 3, 7, 16, 17, 100, 997, 1_000] {
            let indexed = Indexed::new(42, count);
            let mut seen = vec![false; count];
            for index in 0..count {
                let stratum = indexed.stratum(index);
                assert!(stratum < count, "{count}: stratum {stratum} out of range");
                assert!(!seen[stratum], "{count}: stratum {stratum} visited twice");
                seen[stratum] = true;
            }
            assert!(seen.iter().all(|hit| *hit), "{count}: a stratum was missed");
        }
    }

    /// Draws land one to a band, which is the whole point of stratifying.
    #[test]
    fn every_probability_band_receives_one_draw() {
        let count = 1_000;
        let indexed = Indexed::new(7, count);
        let mut occupancy = vec![0_usize; count];
        for index in 0..count {
            let probability = indexed.probability(index);
            occupancy[((probability * count as f64) as usize).min(count - 1)] += 1;
        }
        assert!(occupancy.iter().all(|hits| *hits == 1));
    }

    /// The order is scrambled, so quantities combined at matching indices are not
    /// silently rank-correlated.
    #[test]
    fn draws_are_not_left_in_ascending_order() {
        let indexed = Indexed::new(11, 512);
        let probabilities: Vec<f64> = (0..512).map(|index| indexed.probability(index)).collect();
        assert!(probabilities.windows(2).any(|pair| pair[0] > pair[1]));
    }

    /// Two differently seeded sites must not line up.
    ///
    /// The failure this guards against is silent: a model would still solve, and
    /// every sum of two independent quantities would carry the variance of a sum
    /// of one quantity with itself.
    #[test]
    fn separately_seeded_sites_are_uncorrelated() {
        let count = 4_000;
        let left = Indexed::new(1, count);
        let right = Indexed::new(2, count);
        let (left, right): (Vec<f64>, Vec<f64>) = (0..count)
            .map(|index| (left.probability(index), right.probability(index)))
            .unzip();

        let mean = |values: &[f64]| values.iter().sum::<f64>() / values.len() as f64;
        let (left_mean, right_mean) = (mean(&left), mean(&right));
        let covariance: f64 = left
            .iter()
            .zip(&right)
            .map(|(l, r)| (l - left_mean) * (r - right_mean))
            .sum::<f64>()
            / count as f64;
        let deviation = |values: &[f64], mean: f64| {
            (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64).sqrt()
        };
        let correlation =
            covariance / (deviation(&left, left_mean) * deviation(&right, right_mean));

        // Sorted-but-unpermuted sites would score 1.0 here.
        assert!(
            correlation.abs() < 0.05,
            "sites correlated at {correlation}"
        );
    }

    /// Stratifying is worth keeping: the mean is far better than independent
    /// sampling would give at the same count.
    #[test]
    fn stratification_survives_the_change_of_permutation() {
        let count = 1_000;
        let indexed = Indexed::new(3, count);
        let mean = (0..count)
            .map(|index| indexed.probability(index))
            .sum::<f64>()
            / count as f64;
        // Independent uniforms would have a standard error of 1/sqrt(12n), about
        // 0.009, so this bound would fail about nine times in ten without strata.
        assert!((mean - 0.5).abs() < 0.001, "mean was {mean}");
    }

    #[test]
    #[ignore = "measurement, not an assertion"]
    fn cost_against_materialising() {
        let count = 1_000;
        let rounds = 1_000;

        let indexed = Indexed::new(3, count);
        let started = std::time::Instant::now();
        let mut total = 0.0;
        for _ in 0..rounds {
            for index in 0..count {
                total += indexed.probability(index);
            }
        }
        let on_demand = started.elapsed();

        let started = std::time::Instant::now();
        let mut stored = 0.0;
        for _ in 0..rounds {
            let held: Vec<f64> = (0..count).map(|index| indexed.probability(index)).collect();
            stored += held.iter().sum::<f64>();
        }
        let materialised = started.elapsed();

        println!(
            "on demand {on_demand:?}, materialised {materialised:?} ({total} {stored})\n  \
             per draw: {:?} on demand",
            on_demand / (rounds * count) as u32
        );
    }
}
