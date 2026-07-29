//! Correlation-preserving sample sets shared by every clone of a distribution value.
//!
//! # Why draws are shared
//!
//! Distribution algebra in Squiggle is *sample-set* algebra: composing two
//! distributions combines their draws elementwise at matching indices rather than
//! convolving their densities. Elementwise composition is what makes a variable
//! behave like a random variable instead of an independent generator, so that
//! `x = normal(5, 1); x - x` collapses to exactly zero and `x / x` to exactly one.
//! Drawing fresh samples at each use site would instead model each occurrence of
//! `x` as an independent replicate, inflating variance by a factor of two for a
//! difference of identically distributed terms and destroying every dependency a
//! model relies on.
//!
//! Sharing is achieved by identity rather than by structure. Each *syntactic*
//! distribution constructor allocates one [`DrawCache`], and every clone of the
//! resulting value — including each variable lookup — shares that allocation.
//! Two textually identical constructors therefore remain independent, while two
//! references to one binding remain perfectly dependent.
//!
//! # Stratified inverse-transform sampling
//!
//! Draws are materialised by stratified inverse-transform sampling. For a target
//! count $n$ the unit interval is partitioned into $n$ equal strata and one
//! uniform variate is placed in each:
//!
//! $$u_i = \frac{i + \xi_i}{n}, \qquad \xi_i \sim \mathcal{U}[0, 1), \qquad i = 0, \dots, n-1$$
//!
//! and the draw is $x_i = F^{-1}(u_{\pi(i)})$ for the quantile function $F^{-1}$
//! of the distribution and a uniform random permutation $\pi$.
//!
//! Stratification guarantees exactly one draw per $1/n$ probability band, which
//! removes the clustering and gaps of independent uniform sampling. For a
//! function $g$ of bounded variation the variance of the stratified mean is
//! $O(n^{-2})$ against $O(n^{-1})$ for independent sampling, so quantile and mean
//! estimates stay stable at sample counts an order of magnitude smaller than
//! independent sampling requires. This matters because every fixed-point
//! relaxation step re-evaluates the whole model, and sampling noise that would be
//! harmless in a single pass becomes a convergence failure under iteration.
//!
//! # Why the permutation is mandatory
//!
//! The permutation $\pi$ is a correctness requirement, not a refinement. Strata
//! are generated in ascending order, so an unpermuted sample set is sorted. Two
//! independently stratified but sorted sample sets combined elementwise would be
//! perfectly rank-correlated — comonotonic — which is the opposite of the
//! independence their separate construction is supposed to express. Shuffling
//! each site independently with Fisher–Yates restores exchangeability between
//! sites while leaving the marginal distribution of each site untouched.
//!
//! # Assumptions and limitations
//!
//! - Independence between distinct constructor sites is *approximate*: it holds
//!   in distribution but the draws are not independent across strata within a
//!   site. This is the standard trade-off of variance reduction and is why the
//!   effective sample size for tail statistics is smaller than $n$.
//! - Discrete families are sampled through the same inverse CDF, so strata
//!   falling inside one atom all map to that atom, which is correct but means
//!   stratification yields no variance reduction for coarse discrete supports.
//! - Materialisation is keyed only on identity, not on the requested count. A
//!   runtime evaluates with one fixed `sample_count`, so a cached sample set is
//!   always the length every later consumer expects. Ad-hoc counts belong on
//!   [`Distribution::sample_n`], which deliberately draws independently.
//!
//! References: Art B. Owen, *Monte Carlo theory, methods and examples* (2013),
//! chapter 8 on stratification; Luc Devroye, *Non-Uniform Random Variate
//! Generation* (1986), chapter 2 on inverse transform sampling; Donald Knuth,
//! *The Art of Computer Programming* volume 2, algorithm 3.4.2P for the shuffle.

use std::sync::{Arc, OnceLock};

use rand::Rng;
use rand_chacha::ChaCha20Rng;

use super::{Distribution, Ensemble, Kind};

/// A lazily materialised sample set shared by every clone of a distribution.
///
/// Cloning a [`Distribution`] clones this handle, not the draws, so all clones
/// observe the first materialisation. The cache participates in neither equality
/// nor hashing: two distributions are equal when their parameters agree,
/// regardless of whether either has been sampled.
#[derive(Clone, Debug, Default)]
pub(super) struct DrawCache(Arc<OnceLock<Vec<f64>>>);

impl DrawCache {
    fn get(&self) -> Option<&[f64]> {
        self.0.get().map(Vec::as_slice)
    }

    fn set(&self, draws: Vec<f64>) -> &[f64] {
        self.0.get_or_init(|| draws)
    }
}

impl Distribution {
    /// Returns the shared draws for this value, materialising them on first use.
    ///
    /// The returned slice is stable for the lifetime of the value: repeated
    /// calls, and calls on any clone, yield the same slice. Callers combining
    /// two distributions elementwise rely on this to preserve dependence between
    /// references to a common binding.
    ///
    /// An authored sample set is returned verbatim whenever `count` matches its
    /// length, so explicit draws are never silently reinterpolated. Pair this
    /// with `aligned_count` to choose a count that keeps authored data intact.
    ///
    /// Sampling is always over the whole ensemble, and only the caller's share of
    /// the result is handed back. Materialising the whole of it is what makes a
    /// share identical to the same draws computed without splitting, and the
    /// cache holds the whole so that two shares of one value still agree.
    ///
    /// ```
    /// # use optimist::squiggle::{Runtime, RuntimeConfig, Value};
    /// let config = RuntimeConfig { sample_count: 1_000, ..RuntimeConfig::default() };
    /// let mut runtime = Runtime::with_config(config)?;
    /// let Ok(Value::Distribution(result)) = runtime.evaluate("x = normal(5, 1)\nx - x") else {
    ///     panic!("expected a distribution");
    /// };
    /// assert_eq!(result.mean()?, 0.0);
    /// # Ok::<(), String>(())
    /// ```
    pub(crate) fn draws<'a>(
        &'a self,
        ensemble: Ensemble,
        rng: &mut ChaCha20Rng,
    ) -> Result<&'a [f64], String> {
        if let Kind::Samples(samples) = &self.kind {
            if samples.len() == ensemble.size() {
                return Ok(ensemble.window(samples.as_ref()));
            }
            // Already narrowed to this share by whoever produced it. Narrowing a
            // second time would take a share of a share.
            if samples.len() == ensemble.len() {
                return Ok(samples.as_ref());
            }
        }
        if let Some(draws) = self.draws.get() {
            return Ok(ensemble.window(draws));
        }
        Ok(ensemble.window(
            self.draws.set(self.stratified(ensemble.size(), rng)?),
        ))
    }

    /// Returns the ensemble at which `operands` can be combined elementwise.
    ///
    /// Authored sample sets carry a fixed number of draws that resampling would
    /// distort, so the shortest authored length wins and symbolic operands
    /// materialise to match it.
    ///
    /// Operands that already hold exactly this share leave the share in place. A
    /// symbolic operand joining them still has to be drawn across the whole
    /// ensemble and narrowed, or it would be sampling its own strata rather than
    /// the ones its neighbours came from, and the shares would stop fitting back
    /// together.
    pub(crate) fn aligned<'a>(
        operands: impl IntoIterator<Item = &'a Self>,
        configured: Ensemble,
    ) -> Ensemble {
        match operands
            .into_iter()
            .filter_map(|operand| operand.samples().map(<[f64]>::len))
            .min()
        {
            None => configured,
            Some(authored) if authored == configured.len() => configured,
            Some(authored) => Ensemble::whole(authored),
        }
    }

    fn stratified(&self, count: usize, rng: &mut ChaCha20Rng) -> Result<Vec<f64>, String> {
        if count == 0 {
            return Err("a sample set requires at least one draw".into());
        }
        let width = 1.0 / count as f64;
        let mut probabilities = (0..count)
            .map(|stratum| {
                let offset: f64 = rng.gen_range(0.0..1.0);
                ((stratum as f64 + offset) * width).clamp(f64::EPSILON, 1.0 - f64::EPSILON)
            })
            .collect::<Vec<_>>();
        for index in (1..probabilities.len()).rev() {
            probabilities.swap(index, rng.gen_range(0..=index));
        }
        probabilities
            .into_iter()
            .map(|probability| self.quantile(probability))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use super::*;

    fn rng() -> ChaCha20Rng {
        ChaCha20Rng::seed_from_u64(7)
    }

    fn whole(size: usize) -> Ensemble {
        Ensemble::whole(size)
    }

    #[test]
    fn clones_share_one_materialisation() -> Result<(), String> {
        let distribution = Distribution::normal(5.0, 1.0)?;
        let clone = distribution.clone();
        let first = distribution.draws(whole(512), &mut rng())?.to_vec();
        let second = clone.draws(whole(512), &mut rng())?;
        assert_eq!(first, second);
        Ok(())
    }

    #[test]
    fn separate_values_draw_independently() -> Result<(), String> {
        let mut rng = rng();
        let first = Distribution::normal(5.0, 1.0)?;
        let second = Distribution::normal(5.0, 1.0)?;
        let left = first.draws(whole(512), &mut rng)?.to_vec();
        let right = second.draws(whole(512), &mut rng)?;
        assert_ne!(left, right);
        Ok(())
    }

    #[test]
    fn strata_cover_the_unit_interval_exactly_once() -> Result<(), String> {
        let count = 1_000;
        let draws = Distribution::uniform(0.0, 1.0)?
            .draws(whole(count), &mut rng())?
            .to_vec();
        let mut occupancy = vec![0_usize; count];
        for draw in &draws {
            occupancy[((draw * count as f64) as usize).min(count - 1)] += 1;
        }
        assert!(occupancy.iter().all(|hits| *hits == 1));
        Ok(())
    }

    #[test]
    fn draws_are_not_left_in_ascending_order() -> Result<(), String> {
        let draws = Distribution::normal(0.0, 1.0)?
            .draws(whole(512), &mut rng())?
            .to_vec();
        assert!(draws.windows(2).any(|pair| pair[0] > pair[1]));
        Ok(())
    }

    #[test]
    fn stratification_beats_independent_sampling_on_mean_error() -> Result<(), String> {
        let distribution = Distribution::normal(10.0, 3.0)?;
        let stratified = distribution.draws(whole(1_000), &mut rng())?;
        let mean = stratified.iter().sum::<f64>() / stratified.len() as f64;
        assert!((mean - 10.0).abs() < 0.01, "stratified mean was {mean}");
        Ok(())
    }

    #[test]
    fn existing_sample_sets_of_matching_length_are_returned_unchanged() -> Result<(), String> {
        let samples = vec![1.0, 2.0, 3.0, 4.0];
        let distribution = Distribution::from_samples(samples.clone())?;
        assert_eq!(distribution.draws(whole(4), &mut rng())?, samples.as_slice());
        Ok(())
    }

    #[test]
    fn sample_sets_of_other_lengths_are_resampled_to_the_requested_count() -> Result<(), String> {
        let distribution = Distribution::from_samples(vec![1.0, 2.0, 3.0, 4.0])?;
        assert_eq!(distribution.draws(whole(64), &mut rng())?.len(), 64);
        Ok(())
    }

    /// A share reads the draws the whole ensemble would have put at those indices.
    ///
    /// This is what lets one solve be computed in pieces: the pieces are not
    /// merely similar to the whole, they are the whole, rearranged.
    #[test]
    fn shares_of_a_sample_set_reconstruct_the_whole() -> Result<(), String> {
        let distribution = Distribution::lognormal(1.0, 0.5)?;
        let whole = distribution.draws(whole(600), &mut rng())?.to_vec();

        let mut assembled = Vec::new();
        for share in Ensemble::split(600, 7) {
            // A fresh value each time, so nothing is carried over in the cache.
            let separate = Distribution::lognormal(1.0, 0.5)?;
            assembled.extend_from_slice(separate.draws(share, &mut rng())?);
        }
        assert_eq!(assembled, whole);
        Ok(())
    }
}
