/**
 * Vertical scale for an outcome plotted in its own unit.
 *
 * Improvement-versus-baseline percentages made these charts unreadable: an
 * outcome resting near zero turns any absolute movement into a five-digit
 * percentage, and a minimised outcome getting worse drew a line going down,
 * because improvement is measured against the objective's direction rather than
 * against the quantity. Plotting the quantity itself removes both problems --
 * the axis carries the outcome's unit, and up always means more of it.
 *
 * The range those quantities span is the reason for the logarithm. A failure
 * count that rests near 0.02 req/sec and saturates near 150 covers four decades;
 * on a linear axis the resting behaviour is a flat line on the floor and only the
 * collapse is visible.
 */
export interface OutcomeScale {
  /** Lowest value the axis shows, in the outcome's unit. */
  lower: number
  /** Highest value the axis shows, in the outcome's unit. */
  upper: number
  /**
   * Whether positions are placed by logarithm.
   *
   * A logarithm needs positive values, and an outcome on a real support -- a
   * balance, a margin -- may legitimately sit below zero. Those fall back to a
   * linear axis rather than being clipped away or silently floored.
   */
  logarithmic: boolean
}

/** Fraction of the span added above and below so lines do not touch the frame. */
const PADDING = 0.06

/**
 * Decades a logarithmic axis will show before it stops resolving the floor.
 *
 * A propagated quantity can pass through exact zero, or through a value so small
 * it is numerically indistinguishable from it, and a single such period would
 * otherwise stretch the axis over a dozen decades and flatten everything real
 * into one line. Values below the floor are drawn on it; the tooltip still
 * reports what they were.
 */
const MAX_DECADES = 6

/**
 * Fits an axis to the series being drawn.
 *
 * `primary` decides the shape of the axis and `secondary` may only stretch it.
 * The uncertainty band belongs in `secondary`: a band reaching down to zero says
 * nothing about whether the quantity is positive, and letting it decide would
 * drop the whole chart onto a linear axis over a range that needs a logarithm.
 */
export function outcomeScale(
  primary: Array<number | null | undefined>,
  secondary: Array<number | null | undefined> = [],
): OutcomeScale {
  const lead = finite(primary)
  if (!lead.length) return { lower: 0, upper: 1, logarithmic: false }
  const every = [...lead, ...finite(secondary)]
  const highest = Math.max(...every)
  if (lead.some((value) => value < 0) || highest <= 0) return linear(every)
  const positive = every.filter((value) => value > 0)
  const floor = Math.max(Math.min(...positive), highest / 10 ** MAX_DECADES)
  return logarithmic(floor, highest)
}

/** Places `value` in `[0, 1]`, where 1 is the top of the axis. */
export function positionOn(scale: OutcomeScale, value: number): number {
  if (!Number.isFinite(value)) return 0
  const clamped = Math.min(Math.max(value, scale.lower), scale.upper)
  if (!scale.logarithmic) {
    const span = scale.upper - scale.lower
    return span === 0 ? 0.5 : (clamped - scale.lower) / span
  }
  const span = Math.log10(scale.upper) - Math.log10(scale.lower)
  return span === 0 ? 0.5 : (Math.log10(clamped) - Math.log10(scale.lower)) / span
}

function finite(values: Array<number | null | undefined>): number[] {
  return values.filter(
    (value): value is number => typeof value === 'number' && Number.isFinite(value),
  )
}

function logarithmic(lowest: number, highest: number): OutcomeScale {
  const low = Math.log10(lowest)
  const high = Math.log10(highest)
  const margin = Math.max((high - low) * PADDING, 0.05)
  return { lower: 10 ** (low - margin), upper: 10 ** (high + margin), logarithmic: true }
}

function linear(values: number[]): OutcomeScale {
  const lowest = Math.min(...values)
  const highest = Math.max(...values)
  const margin = Math.max((highest - lowest) * PADDING, Math.abs(highest) * 0.05, 1e-9)
  return { lower: lowest - margin, upper: highest + margin, logarithmic: false }
}
