//! Correlation-preserving draws, computed from where they sit rather than stored.
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
//! distribution constructor allocates one [`Stream`], and every clone of the
//! resulting value — including each variable lookup — shares that allocation and
//! therefore its seed. Two textually identical constructors remain independent,
//! while two references to one binding remain perfectly dependent.
//!
//! # Stratified inverse-transform sampling
//!
//! For a target count $n$ the unit interval is partitioned into $n$ equal strata
//! and one uniform variate is placed in each:
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
//! independence their separate construction is supposed to express. Permuting
//! each site independently restores exchangeability between sites while leaving
//! the marginal distribution of each site untouched.
//!
//! # Why nothing is stored
//!
//! Both properties above describe a draw's *position*, not its neighbours, so
//! neither needs the set to exist. $\pi$ is evaluated on demand by
//! [`Indexed`](super::indexed) rather than produced by shuffling an array, and
//! $\xi$ follows from the seed and the stratum. A value therefore carries a seed
//! rather than a sample set: eight bytes instead of eight kilobytes, and no
//! materialisation to synchronise between clones.
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
//! - A draw is recomputed each time it is asked for. Inverting a quantile costs
//!   more than reading an array would, and callers that read one value many times
//!   should hold it. Ad-hoc counts belong on [`Distribution::sample_n`], which
//!   deliberately draws independently.
//!
//! References: Art B. Owen, *Monte Carlo theory, methods and examples* (2013),
//! chapter 8 on stratification; Luc Devroye, *Non-Uniform Random Variate
//! Generation* (1986), chapter 2 on inverse transform sampling.


use std::sync::{Arc, OnceLock};

use rand::RngCore;
use rand_chacha::ChaCha20Rng;

use super::{Distribution, Ensemble, Kind, indexed::Indexed};

/// The stream a distribution's draws are taken from, shared by every clone.
///
/// Cloning a [`Distribution`] clones this handle, so every clone draws from one
/// stream and produces identical values at identical indices. That is what makes
/// `x - x` exactly zero while two textually identical constructors stay
/// independent: they are separate handles, and are seeded separately.
///
/// The seed is taken from the run's generator on first use rather than at
/// construction. A pointer address would identify a value just as well, but it
/// would differ between runs of the same model, and identical source is required
/// to replay exactly. Deferring it also keeps the public constructors free of a
/// generator they have no way to reach.
///
/// The handle participates in neither equality nor hashing: two distributions are
/// equal when their parameters agree, regardless of whether either has drawn.
///
/// Both cells live behind one allocation. Every result of distribution algebra
/// creates a stream, so a second allocation and a second reference count per
/// value are paid tens of thousands of times per solve for two words.
#[derive(Clone, Debug, Default)]
pub(super) struct Stream(Arc<Shared>);

#[derive(Debug, Default)]
struct Shared {
    seed: OnceLock<u64>,
    /// The share this value has already been drawn for, if any.
    ///
    /// Inverting a quantile costs far more than reading one back, and a property
    /// is read by every channel that names it on every pass. Until the solver
    /// iterates draws on the outside — where one index is fixed for a whole
    /// relaxation and a single slot would serve — the share is worth keeping.
    /// It is the draws of a few dozen authored quantities, not of the thousands
    /// of values derived from them.
    held: OnceLock<(Ensemble, Arc<[f64]>)>,
}

impl Stream {
    fn seed(&self, rng: &mut ChaCha20Rng) -> u64 {
        *self.0.seed.get_or_init(|| rng.next_u64())
    }

    fn held(&self) -> &OnceLock<(Ensemble, Arc<[f64]>)> {
        &self.0.held
    }
}

impl Distribution {
    /// Seeds this value's draws, or reports the seed it already has.
    ///
    /// Every clone of one value answers with one seed and separate values answer
    /// with separate ones, so a caller holds the answer for as long as it is
    /// reading draws and the whole sample set comes from one stream.
    pub(crate) fn stream(&self, rng: &mut ChaCha20Rng) -> u64 {
        self.stream.seed(rng)
    }

    /// One draw, computed from where it sits rather than read from a stored set.
    ///
    /// `index` counts within the caller's share; which draw of the whole ensemble
    /// that is follows from the share. An authored sample set is indexed
    /// directly, because its values are data rather than something to generate.
    /// This value's share of the ensemble, drawn once and kept.
    ///
    /// Callers hold the result for as long as they are reading from it. Inverting
    /// a quantile costs far more than reading one back, and reaching through the
    /// distribution for each draw costs more again, so the share is resolved once
    /// and indexed thereafter.
    pub(crate) fn drawn(&self, seed: u64, ensemble: Ensemble) -> Arc<[f64]> {
        if let Kind::Samples(samples) = &self.kind {
            let held = self.held(samples, ensemble);
            return if held.len() == samples.len() {
                Arc::clone(samples)
            } else {
                held.into_owned().into()
            };
        }
        if let Some((held, draws)) = self.stream.held().get()
            && *held == ensemble
        {
            return Arc::clone(draws);
        }
        let drawn: Arc<[f64]> = self.compute(seed, ensemble).into();
        let (held, kept) = self
            .stream
            .held()
            .get_or_init(|| (ensemble, Arc::clone(&drawn)));
        if *held == ensemble {
            return Arc::clone(kept);
        }
        drawn
    }

    fn compute(&self, seed: u64, ensemble: Ensemble) -> Vec<f64> {
        let drawn = Indexed::new(seed, ensemble.size());
        ensemble
            .indices(ensemble.size())
            .map(|draw| {
                // Parameters are validated at construction and the probability is
                // clamped inside the open unit interval, so nothing is left to fail.
                self.quantile(drawn.probability(draw)).unwrap_or(f64::NAN)
            })
            .collect()
    }

    /// The part of an authored set this share reads.
    ///
    /// A set as long as the whole ensemble is narrowed to the share. One that is
    /// already the share's length was narrowed by whoever produced it, and
    /// narrowing again would take a share of a share.
    fn held<'a>(&self, samples: &'a [f64], ensemble: Ensemble) -> std::borrow::Cow<'a, [f64]> {
        if samples.len() == ensemble.size() {
            return ensemble.window(samples);
        }
        let whole_share = ensemble.retaining(u64::MAX);
        if ensemble.live() != u64::MAX && samples.len() == whole_share.len() {
            return std::borrow::Cow::Owned(
                ensemble
                    .positions(ensemble.size())
                    .map(|position| samples[position])
                    .collect(),
            );
        }
        std::borrow::Cow::Borrowed(samples)
    }

    /// This value's whole share of the ensemble, gathered into one array.
    ///
    /// For the callers that genuinely need every draw at once: summarising a
    /// sample set, or handing one to something that only speaks in slices. The
    /// solver reads draws one at a time and does not come through here.
    pub(crate) fn materialise(&self, seed: u64, ensemble: Ensemble) -> Vec<f64> {
        self.drawn(seed, ensemble).to_vec()
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
            Some(authored) if authored == configured.retaining(u64::MAX).len() => configured,
            // A set as long as the whole ensemble is this share's own quantity
            // seen at full width, so it is narrowed to the share rather than
            // mistaken for authored data that fixes its own draw count.
            Some(authored) if authored == configured.size() => configured,
            Some(authored) => Ensemble::whole(authored),
        }
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

    fn drawn(distribution: &Distribution, size: usize) -> Vec<f64> {
        let seed = distribution.stream(&mut rng());
        distribution.materialise(seed, whole(size))
    }

    #[test]
    fn clones_share_one_stream() -> Result<(), String> {
        let distribution = Distribution::normal(5.0, 1.0)?;
        let clone = distribution.clone();
        assert_eq!(drawn(&distribution, 512), drawn(&clone, 512));
        Ok(())
    }

    #[test]
    fn separate_values_draw_independently() -> Result<(), String> {
        let mut rng = rng();
        let first = Distribution::normal(5.0, 1.0)?;
        let second = Distribution::normal(5.0, 1.0)?;
        let (left, right) = (first.stream(&mut rng), second.stream(&mut rng));
        assert_ne!(
            first.materialise(left, whole(512)),
            second.materialise(right, whole(512))
        );
        Ok(())
    }

    #[test]
    fn strata_cover_the_unit_interval_exactly_once() -> Result<(), String> {
        let count = 1_000;
        let draws = drawn(&Distribution::uniform(0.0, 1.0)?, count);
        let mut occupancy = vec![0_usize; count];
        for draw in &draws {
            occupancy[((draw * count as f64) as usize).min(count - 1)] += 1;
        }
        assert!(occupancy.iter().all(|hits| *hits == 1));
        Ok(())
    }

    #[test]
    fn draws_are_not_left_in_ascending_order() -> Result<(), String> {
        let draws = drawn(&Distribution::normal(0.0, 1.0)?, 512);
        assert!(draws.windows(2).any(|pair| pair[0] > pair[1]));
        Ok(())
    }

    #[test]
    fn stratification_beats_independent_sampling_on_mean_error() -> Result<(), String> {
        let draws = drawn(&Distribution::normal(10.0, 3.0)?, 1_000);
        let mean = draws.iter().sum::<f64>() / draws.len() as f64;
        assert!((mean - 10.0).abs() < 0.01, "stratified mean was {mean}");
        Ok(())
    }

    #[test]
    fn existing_sample_sets_are_returned_unchanged() -> Result<(), String> {
        let samples = vec![1.0, 2.0, 3.0, 4.0];
        let distribution = Distribution::from_samples(samples.clone())?;
        assert_eq!(drawn(&distribution, 4), samples);
        Ok(())
    }

    /// The property draw retirement rests on: narrowing a share to some of its
    /// blocks selects draws without redrawing them, so a value that survives
    /// retirement is bit-identical to the one it would have had.
    #[test]
    fn retiring_blocks_selects_draws_rather_than_resampling_them() -> Result<(), String> {
        let count = 1_000;
        let distribution = Distribution::normal(10.0, 3.0)?;
        let seed = distribution.stream(&mut rng());
        let every = distribution.materialise(seed, whole(count));

        let live = 0x0F0F_0F0F_0F0F_0F0F;
        let share = whole(count).retaining(live);
        let kept = distribution.materialise(seed, share);

        let expected: Vec<f64> = share.indices(count).map(|draw| every[draw]).collect();
        assert_eq!(kept, expected);
        assert_eq!(kept.len(), share.width(count));
        Ok(())
    }

    /// Authored sample sets are narrowed by the same mask as symbolic ones, so a
    /// constant and a solved quantity stay aligned once blocks have retired.
    #[test]
    fn retiring_blocks_narrows_an_authored_set_the_same_way() -> Result<(), String> {
        let samples: Vec<f64> = (0..64).map(f64::from).collect();
        let distribution = Distribution::from_samples(samples.clone())?;
        let share = whole(64).retaining(0b1011);
        let seed = distribution.stream(&mut rng());
        assert_eq!(distribution.materialise(seed, share), vec![0.0, 1.0, 3.0]);
        Ok(())
    }

    #[test]
    fn retiring_blocks_narrows_a_sample_set_that_already_holds_one_share() -> Result<(), String> {
        let samples: Vec<f64> = (0..32).map(f64::from).collect();
        let distribution = Distribution::from_samples(samples.clone())?;
        let share = Ensemble::split(64, 2)
            .nth(1)
            .expect("two shares")
            .retaining(0x0F0F_0F0F_0F0F_0F0F);
        let expected = share
            .positions(64)
            .map(|position| samples[position])
            .collect::<Vec<_>>();
        let seed = distribution.stream(&mut rng());
        assert_eq!(distribution.materialise(seed, share), expected);
        Ok(())
    }

    /// A share reads the draws the whole ensemble would have put at those indices.
    #[test]
    fn shares_of_a_sample_set_reconstruct_the_whole() -> Result<(), String> {
        let distribution = Distribution::lognormal(1.0, 0.5)?;
        let seed = distribution.stream(&mut rng());
        let whole = distribution.materialise(seed, Ensemble::whole(600));

        let assembled: Vec<f64> = Ensemble::split(600, 7)
            .flat_map(|share| distribution.materialise(seed, share))
            .collect();
        assert_eq!(assembled, whole);
        Ok(())
    }

    /// Draws no longer need storing, so reading them twice must still agree.
    #[test]
    fn a_draw_is_the_same_every_time_it_is_asked_for() -> Result<(), String> {
        let distribution = Distribution::gamma(2.0, 3.0)?;
        let seed = distribution.stream(&mut rng());
        let ensemble = whole(256);
        assert_eq!(
            distribution.drawn(seed, ensemble),
            distribution.drawn(seed, ensemble)
        );
        Ok(())
    }
}
