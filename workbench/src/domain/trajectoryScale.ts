/**
 * Y-axis scaling for objective trajectories.
 *
 * A projection can hold a few percent for eleven periods and then carry an
 * uncertainty band hundreds of times wider for one, when a rebound lands on a
 * state that several relationships all reach at once. On an axis sized to that
 * band, every mean collapses onto the zero line and the chart says nothing
 * about the plan it is meant to explain.
 */

/** Band extent must exceed this multiple of its p10–p90 extent before clipping. */
const CLIP_RATIO = 3

/** Fraction of the axis added as breathing room above and below the data. */
const PADDING = 0.12

/** One trajectory's plotted values, split into signal and uncertainty. */
export interface TrajectorySamples {
  /** Period means. This is the signal, and it is never clipped. */
  means: number[]
  /** Lower confidence bound per period. */
  lower: number[]
  /** Upper confidence bound per period. */
  upper: number[]
}

export interface TrajectoryScale {
  /** Lowest value the axis covers. */
  lower: number
  /** Highest value the axis covers. */
  upper: number
  /** Half-width of the near-linear region either side of zero. */
  linthresh: number
  /** Whether the confidence band was cut back so the means stay legible. */
  clipped: boolean
}

/**
 * Reads a percentile off sorted values, interpolating between order statistics.
 */
export function percentile(sorted: number[], fraction: number): number {
  if (!sorted.length) return 0
  const position = (sorted.length - 1) * Math.min(Math.max(fraction, 0), 1)
  const lower = Math.floor(position)
  const upper = Math.ceil(position)
  if (lower === upper) return sorted[lower]!
  return sorted[lower]! + (sorted[upper]! - sorted[lower]!) * (position - lower)
}

/**
 * Symmetric log: logarithmic in the tails, near-linear around zero.
 *
 * A plain log cannot render zero or the negative half of an improvement axis,
 * both of which this chart needs. Symlog keeps the sign, compresses magnitude by
 * `sign(v)·ln(1 + |v|/L)`, and degrades to linear as `L` grows past the data, so
 * a chart with a modest range looks exactly as it did before.
 */
export function symlog(value: number, linthresh: number): number {
  return Math.sign(value) * Math.log1p(Math.abs(value) / linthresh)
}

/** Maps a value onto `0` at the axis floor and `1` at its ceiling. */
export function normalize(value: number, scale: TrajectoryScale): number {
  const floor = symlog(scale.lower, scale.linthresh)
  const ceiling = symlog(scale.upper, scale.linthresh)
  if (!(ceiling > floor)) return 0.5
  return (symlog(value, scale.linthresh) - floor) / (ceiling - floor)
}

function finite(values: number[]): number[] {
  return values.filter((value) => Number.isFinite(value)).sort((a, b) => a - b)
}

/**
 * Chooses an axis that shows the trajectory rather than its widest band.
 *
 * The means and the baseline at zero always fit: they are what the reader came
 * for, and a period that genuinely regresses by 87% is the most important thing
 * on the chart rather than an outlier to hide. Only the confidence band is
 * negotiable. When the band spans more than `CLIP_RATIO` times its own p10–p90
 * extent, a couple of very uncertain periods are setting the scale for all the
 * others, so the axis stops at that p10–p90 instead. The excluded band still
 * draws and leaves the frame, which reads as running off the chart rather than
 * being silently flattened.
 *
 * `linthresh` is the median magnitude on the axis, so half the data sits in the
 * linear region and only the extremes are compressed.
 */
export function trajectoryScale(samples: TrajectorySamples): TrajectoryScale {
  const means = finite(samples.means)
  const lowerBounds = finite(samples.lower)
  const upperBounds = finite(samples.upper)
  if (!means.length && !lowerBounds.length && !upperBounds.length) {
    return { lower: -1, upper: 1, linthresh: 1, clipped: false }
  }

  // Zero is the baseline every point is measured against, so it belongs to the
  // signal rather than being something the axis may drop.
  const signalLow = Math.min(means[0] ?? 0, 0)
  const signalHigh = Math.max(means.at(-1) ?? 0, 0)
  const bandLow = lowerBounds[0] ?? signalLow
  const bandHigh = upperBounds.at(-1) ?? signalHigh
  const clipLow = lowerBounds.length ? percentile(lowerBounds, 0.1) : bandLow
  const clipHigh = upperBounds.length ? percentile(upperBounds, 0.9) : bandHigh
  const clipExtent = clipHigh - clipLow
  const clipped = clipExtent > 0 && bandHigh - bandLow > CLIP_RATIO * clipExtent

  const low = Math.min(signalLow, clipped ? clipLow : bandLow)
  const high = Math.max(signalHigh, clipped ? clipHigh : bandHigh)
  const padding = Math.max((high - low) * PADDING, 0.01)
  const lower = low - padding
  const upper = high + padding

  const magnitudes = [...means, ...lowerBounds, ...upperBounds]
    .filter((value) => value !== 0 && value >= lower && value <= upper)
    .map(Math.abs)
    .sort((a, b) => a - b)
  const linthresh = Math.max(
    magnitudes.length ? percentile(magnitudes, 0.5) : (upper - lower) / 10,
    (upper - lower) * 1e-4,
    Number.EPSILON,
  )
  return { lower, upper, linthresh, clipped }
}
