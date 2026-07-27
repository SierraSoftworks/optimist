/**
 * Kernel density estimation for solved quantities.
 *
 * # What this has to get right
 *
 * A relaxation solved over aligned draws settles each draw on its own fixed
 * point, so a design sitting near a fold returns a genuine mixture: some draws
 * healthy, some collapsed. That a result has two branches rather than one wide
 * spread is usually the most important thing about it, and it is invisible in
 * every summary the server also sends — a mean, a median and a percentile pair
 * describe a bimodal sample and a broad unimodal one identically. This estimate
 * is the only place that distinction survives, so the requirement on it is not
 * smoothness but honesty about how many modes there are.
 *
 * # Bandwidth
 *
 * The rule-of-thumb bandwidth
 *
 * $$h = 0.9\,\hat{\sigma}\,n^{-1/5},\qquad
 *   \hat{\sigma} = \min\!\left(s, \frac{\mathrm{IQR}}{1.349}\right)$$
 *
 * minimises asymptotic mean integrated squared error for normal data
 * (Silverman 1986, §3.4.2), and is known to oversmooth densities that are not
 * normal. That is a real objection to using it here, and it turns out not to bite
 * at the separations this tool produces: $h$ grows only with $\hat{\sigma}$, while
 * the distance between two branches grows with the gap itself, so a mixture wide
 * enough to be worth reporting has components several $h$ apart.
 *
 * An earlier version searched downward from $h$ for the *critical bandwidth*, the
 * largest bandwidth whose estimate has two modes, following Silverman's (1981)
 * test for multimodality. It was removed after measurement. At the sample sizes
 * the server sends, narrowing the kernel far enough to uncover a faint mode also
 * uncovers ripples that are sampling noise, and the search reported extra modes
 * for plainly unimodal samples more often than it rescued a real one. The mode
 * count below does that work instead, at a bandwidth that can be justified.
 *
 * # Limitations
 *
 * Components separated by less than about $2\sigma$ are not resolved. Such a
 * mixture has no visible dip in its true density either, so this is a limit on
 * what can be claimed rather than on the method.
 *
 * The thresholds governing what counts as a mode are judgements about what is
 * worth drawing, not statistical statements. Nothing here produces a p-value and
 * none of it should be read as a test.
 */

/** Points on the evaluation grid. Enough to draw smoothly, cheap enough to redraw. */
const GRID_POINTS = 160

/** How far past the extreme draws the grid runs, in bandwidths. */
const TAIL_BANDWIDTHS = 3

/**
 * How deep the valley between two peaks must be to count them as separate modes,
 * as a fraction of the shorter peak.
 *
 * Measured against the shorter peak rather than the tallest one so that a
 * genuine minority mode — the tenth of draws that collapsed — is not discarded
 * for being short. For two components that barely overlap the valley falls to
 * nearly zero and the ratio approaches one; for a wobble in the shoulder of a
 * single peak it stays near zero.
 */
const MIN_PROMINENCE = 0.3

/**
 * How tall a peak must be to be a mode at all, as a fraction of the tallest.
 *
 * Separation alone is not enough. Out in a tail the density is near zero, so two
 * adjacent ripples there are separated by a valley that is deep *relative to
 * them* and invisible relative to the distribution. Without this the ripples
 * every kernel estimate has in its tails would each be announced as a branch of
 * the design.
 *
 * The floor is low enough to keep a real minority mode: a component holding a
 * tenth of the draws peaks at about a tenth of the majority's height, well above
 * this, while tail ripples sit an order of magnitude below it.
 */
const MIN_MODE_HEIGHT = 0.05

/** A density estimate over a grid. */
export interface Density {
  /** Grid positions. */
  x: number[]
  /** Estimated density at each position. */
  y: number[]
  /** Bandwidth used. */
  bandwidth: number
  /** Modes found. More than one means the design has settled on several states. */
  modes: number
}

/** Quantile of a sorted sample, by linear interpolation between order statistics. */
export function quantileOf(sorted: number[], p: number): number {
  if (sorted.length === 0) return Number.NaN
  if (sorted.length === 1) return sorted[0]
  const position = (sorted.length - 1) * p
  const lower = Math.floor(position)
  const upper = Math.ceil(position)
  if (lower === upper) return sorted[lower]
  return sorted[lower] + (position - lower) * (sorted[upper] - sorted[lower])
}

/** Estimates the density of a sample, or returns null where there is none to estimate. */
export function kernelDensity(draws: number[]): Density | null {
  const sample = draws.filter(Number.isFinite).sort((a, b) => a - b)
  if (sample.length < 2) return null

  // Every draw identical: a quantity pinned at a limit. There is no density, and
  // a bandwidth of zero would divide by zero below. The caller draws a point.
  if (sample[sample.length - 1] - sample[0] <= 0) return null

  const bandwidth = ruleOfThumb(sample)
  if (!(bandwidth > 0)) return null

  const y = evaluate(sample, bandwidth)
  return { x: grid(sample, bandwidth), y, bandwidth, modes: Math.max(1, countModes(y)) }
}

/** Silverman's rule of thumb with a robust scale estimate. */
function ruleOfThumb(sorted: number[]): number {
  const n = sorted.length
  const mean = sorted.reduce((total, value) => total + value, 0) / n
  const variance = sorted.reduce((total, value) => total + (value - mean) ** 2, 0) / (n - 1)
  const deviation = Math.sqrt(Math.max(variance, 0))
  const iqr = quantileOf(sorted, 0.75) - quantileOf(sorted, 0.25)
  // The IQR term is skipped rather than allowed to win when it is zero, which
  // happens when more than half the draws sit on one value — a quantity pinned
  // at its limit in most but not all draws.
  const scale = iqr > 0 ? Math.min(deviation, iqr / 1.349) : deviation
  return 0.9 * scale * n ** (-1 / 5)
}

function grid(sorted: number[], bandwidth: number): number[] {
  const from = sorted[0] - TAIL_BANDWIDTHS * bandwidth
  const to = sorted[sorted.length - 1] + TAIL_BANDWIDTHS * bandwidth
  const step = (to - from) / (GRID_POINTS - 1)
  return Array.from({ length: GRID_POINTS }, (_, index) => from + index * step)
}

/**
 * Evaluates the Gaussian kernel estimate
 * $\hat{f}(x) = \frac{1}{nh}\sum_i \phi\!\left(\frac{x - X_i}{h}\right)$
 * on the grid.
 */
function evaluate(sorted: number[], bandwidth: number): number[] {
  const positions = grid(sorted, bandwidth)
  const scale = 1 / (sorted.length * bandwidth * Math.sqrt(2 * Math.PI))
  return positions.map((x) => {
    let total = 0
    for (const draw of sorted) {
      const z = (x - draw) / bandwidth
      // Beyond four bandwidths the kernel contributes under 0.03% of its peak,
      // which is below the resolution of any chart this feeds.
      if (z > -4 && z < 4) total += Math.exp(-0.5 * z * z)
    }
    return total * scale
  })
}

/**
 * Counts modes: peaks with the substance required by {@link MIN_MODE_HEIGHT} and
 * the separation required by {@link MIN_PROMINENCE}.
 */
function countModes(y: number[]): number {
  const tallest = Math.max(...y)
  if (!(tallest > 0)) return 0

  const peaks: number[] = []
  for (let index = 1; index < y.length - 1; index += 1) {
    const isPeak = y[index] > y[index - 1] && y[index] >= y[index + 1]
    if (isPeak && y[index] / tallest >= MIN_MODE_HEIGHT) peaks.push(index)
  }
  if (peaks.length <= 1) return peaks.length

  // Walk the peaks in order against the last one accepted. A shoulder that fails
  // the test is absorbed into the peak before it rather than shifting the
  // comparison along, so a long ripple cannot accumulate into a mode count one
  // wobble at a time.
  let kept = 1
  let reference = peaks[0]
  for (let position = 1; position < peaks.length; position += 1) {
    const current = peaks[position]
    let valley = Number.POSITIVE_INFINITY
    for (let index = reference; index <= current; index += 1) valley = Math.min(valley, y[index])
    const shorter = Math.min(y[reference], y[current])
    if (shorter > 0 && (shorter - valley) / shorter >= MIN_PROMINENCE) {
      kept += 1
      reference = current
    } else if (y[current] > y[reference]) {
      reference = current
    }
  }
  return kept
}
