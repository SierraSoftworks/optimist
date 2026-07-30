//! Counting the states a solved quantity has settled between.
//!
//! # What this is for
//!
//! A relaxation carries a thousand draws at once and settles each on its own
//! fixed point. Where a design sits past a fold, some draws settle on a healthy
//! branch and the rest on a collapsed one, and the result is a genuine mixture
//! rather than one answer with a wide error bar. Every summary the solver
//! otherwise reports — a mean, a median, a percentile pair — describes a bimodal
//! sample and a broad unimodal one identically, so the count below is the only
//! place that distinction survives on the way out.
//!
//! # Method
//!
//! Modes are counted from a Gaussian kernel density estimate
//!
//! $$\hat{f}(x) = \frac{1}{nh}\sum_{i=1}^{n} \phi\!\left(\frac{x - X_i}{h}\right)$$
//!
//! evaluated on a regular grid spanning the sample and three bandwidths past
//! each end, with the kernel truncated beyond four bandwidths where it
//! contributes under 0.03% of its peak.
//!
//! The bandwidth is Silverman's rule of thumb with a robust scale estimate
//!
//! $$h = 0.9\,\hat{\sigma}\,n^{-1/5},\qquad
//!   \hat{\sigma} = \min\!\left(s, \frac{\mathrm{IQR}}{1.349}\right)$$
//!
//! which minimises asymptotic mean integrated squared error for normal data
//! (Silverman 1986, §3.4.2) and is known to oversmooth densities that are not
//! normal. That objection does not bite here: $h$ grows only with
//! $\hat{\sigma}$, while the distance between two branches grows with the gap
//! itself, so a mixture wide enough to be worth reporting has its components
//! several $h$ apart.
//!
//! # Assumptions, limitations and what this is not
//!
//! - Components separated by less than about $2\sigma$ are not resolved. Such a
//!   mixture has no visible dip in its true density either, so this is a limit
//!   on what can be claimed rather than on the method.
//! - The prominence and height thresholds are judgements about what is worth
//!   reporting, not statistical statements. Nothing here produces a p-value and
//!   none of it should be read as a test of multimodality; Silverman's (1981)
//!   critical-bandwidth test is the tool for that question and needs a bootstrap
//!   this cannot afford inside a relaxation.
//! - The estimate is over draws that are stratified rather than independent, so
//!   the effective sample size in the tails is smaller than $n$. The height
//!   threshold below is what keeps that from being announced as a branch.
//!
//! The workbench draws its own estimate of the same shape in
//! `workbench/src/domain/density.ts` and uses the same bandwidth and the same
//! two thresholds. The two are expected to agree; they are separate because one
//! reports and the other draws.

/// Points on the evaluation grid. Enough to separate branches, cheap enough to
/// run once per step of a horizon.
const GRID_POINTS: usize = 160;

/// How far past the extreme draws the grid runs, in bandwidths.
const TAIL_BANDWIDTHS: f64 = 3.0;

/// Bandwidths past which the Gaussian kernel is treated as zero.
const KERNEL_REACH: f64 = 4.0;

/// How deep the valley between two peaks must be to count them as separate
/// states, as a fraction of the shorter peak.
///
/// Measured against the shorter peak so that a genuine minority state — the
/// tenth of draws that collapsed — is not discarded for being short.
const MIN_PROMINENCE: f64 = 0.3;

/// How tall a peak must be to be a state at all, as a fraction of the tallest.
///
/// Separation alone is not enough. Out in a tail the density is near zero, so
/// two adjacent ripples there are separated by a valley that is deep relative to
/// them and invisible relative to the distribution.
const MIN_MODE_HEIGHT: f64 = 0.05;

/// Counts the states a sample has settled between.
///
/// Returns one for a sample with no spread, and one for any sample too small to
/// estimate a density from, because a single state is the claim that asserts
/// least.
pub(super) fn modes(draws: &[f64]) -> usize {
    let mut sample: Vec<f64> = draws
        .iter()
        .copied()
        .filter(|draw| draw.is_finite())
        .collect();
    if sample.len() < 2 {
        return 1;
    }
    sample.sort_by(f64::total_cmp);
    let bandwidth = rule_of_thumb(&sample);
    if !(bandwidth > 0.0) {
        return 1;
    }
    count(&density(&sample, bandwidth)).max(1)
}

/// Silverman's rule of thumb with a robust scale estimate.
fn rule_of_thumb(sorted: &[f64]) -> f64 {
    let n = sorted.len() as f64;
    let mean = sorted.iter().sum::<f64>() / n;
    let variance = sorted.iter().map(|draw| (draw - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let deviation = variance.max(0.0).sqrt();
    let iqr = quantile(sorted, 0.75) - quantile(sorted, 0.25);
    // The interquartile term is skipped rather than allowed to win when it is
    // zero, which happens when more than half the draws sit on one value: a
    // quantity pinned at its limit in most but not all draws.
    let scale = if iqr > 0.0 {
        deviation.min(iqr / 1.349)
    } else {
        deviation
    };
    0.9 * scale * n.powf(-0.2)
}

/// Quantile of a sorted sample, interpolating between order statistics.
fn quantile(sorted: &[f64], probability: f64) -> f64 {
    let position = (sorted.len() - 1) as f64 * probability;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        return sorted[lower];
    }
    sorted[lower] + (position - lower as f64) * (sorted[upper] - sorted[lower])
}

/// Evaluates the kernel estimate on the grid, unnormalised: only the shape of
/// the curve decides the count, so the constant factor is left off.
fn density(sorted: &[f64], bandwidth: f64) -> Vec<f64> {
    let from = sorted[0] - TAIL_BANDWIDTHS * bandwidth;
    let to = sorted[sorted.len() - 1] + TAIL_BANDWIDTHS * bandwidth;
    let step = (to - from) / (GRID_POINTS - 1) as f64;
    (0..GRID_POINTS)
        .map(|index| {
            let x = from + index as f64 * step;
            sorted
                .iter()
                .map(|draw| (x - draw) / bandwidth)
                .filter(|z| z.abs() < KERNEL_REACH)
                .map(|z| (-0.5 * z * z).exp())
                .sum::<f64>()
        })
        .collect()
}

/// Counts peaks with the substance and the separation the thresholds ask for.
fn count(curve: &[f64]) -> usize {
    let tallest = curve.iter().copied().fold(0.0_f64, f64::max);
    if !(tallest > 0.0) {
        return 0;
    }
    let peaks: Vec<usize> = (1..curve.len() - 1)
        .filter(|&index| curve[index] > curve[index - 1] && curve[index] >= curve[index + 1])
        .filter(|&index| curve[index] / tallest >= MIN_MODE_HEIGHT)
        .collect();
    if peaks.len() <= 1 {
        return peaks.len();
    }
    // Each peak is judged against the last one accepted. A shoulder that fails
    // is absorbed into the peak before it rather than shifting the comparison
    // along, so a long ripple cannot accumulate into a count one wobble at a time.
    let mut kept = 1;
    let mut reference = peaks[0];
    for &current in &peaks[1..] {
        let valley = curve[reference..=current]
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let shorter = curve[reference].min(curve[current]);
        if shorter > 0.0 && (shorter - valley) / shorter >= MIN_PROMINENCE {
            kept += 1;
            reference = current;
        } else if curve[current] > curve[reference] {
            reference = current;
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::modes;

    /// Box–Muller normals from a fixed linear congruential stream, so a failure
    /// is reproducible rather than a matter of which seed the day started on.
    fn normals(count: usize, mean: f64, deviation: f64, seed: u64) -> Vec<f64> {
        let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let mut next = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((state >> 11) as f64 / (1_u64 << 53) as f64).max(f64::MIN_POSITIVE)
        };
        (0..count)
            .map(|_| {
                let (first, second) = (next(), next());
                mean + deviation
                    * (-2.0 * first.ln()).sqrt()
                    * (std::f64::consts::TAU * second).cos()
            })
            .collect()
    }

    #[test]
    fn a_sample_with_no_spread_has_one_state() {
        assert_eq!(modes(&[0.4; 500]), 1);
        assert_eq!(modes(&[]), 1);
        assert_eq!(modes(&[7.0]), 1);
    }

    #[test]
    fn one_population_is_one_state() {
        for seed in 1..8 {
            assert_eq!(modes(&normals(600, 10.0, 2.0, seed)), 1, "seed {seed}");
        }
    }

    #[test]
    fn two_separated_populations_are_two_states() {
        let mut draws = normals(400, 0.0, 1.0, 11);
        draws.extend(normals(400, 12.0, 1.0, 12));
        assert_eq!(modes(&draws), 2);
    }

    #[test]
    fn a_minority_branch_is_still_a_state() {
        let mut draws = normals(900, 0.02, 0.01, 21);
        draws.extend(normals(100, 0.9, 0.03, 22));
        assert_eq!(modes(&draws), 2);
    }

    #[test]
    fn overlapping_populations_are_not_split() {
        let mut draws = normals(500, 0.0, 1.0, 31);
        draws.extend(normals(500, 0.7, 1.0, 32));
        assert_eq!(modes(&draws), 1);
    }
}
