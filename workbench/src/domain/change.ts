import { formatSiNumber } from './humanNumber'

/**
 * Below this a difference is the solver's own noise, not an effect.
 *
 * Two Monte Carlo runs of the same design disagree in the last fraction of a
 * percent. Reporting that as a change invites a reader to explain something that
 * did not happen.
 */
export const NOTICEABLE = 0.01

/**
 * How a proportional change reads.
 *
 * A share is the right form while the two numbers are within an order of
 * magnitude of each other, and useless outside it. A success rate that went from
 * a millionth to nearly one is a hundred million per cent higher, which is
 * arithmetically true and tells a reader nothing they can hold; a latency that
 * fell by the same factor reads as "−100%", which is not wrong but cannot be
 * told apart from one that merely fell to nothing.
 *
 * Past a ten-fold difference either way this therefore switches to a multiple,
 * which is the form people actually use for changes that large.
 *
 * A baseline of exactly zero has no proportional change at all — every increase
 * is infinitely many times it — so that case is named rather than divided.
 */
export function describeChange(before: number, after: number): string | null {
  if (!Number.isFinite(before) || !Number.isFinite(after)) return null
  if (before === 0) {
    if (after === 0) return null
    return after > 0 ? 'from nothing' : 'to nothing'
  }

  const share = (after - before) / Math.abs(before)
  if (Math.abs(share) < NOTICEABLE) return null

  // Only meaningful while both are on the same side of zero; a quantity that
  // crossed it has no ratio worth printing.
  const ratio = after / before
  if (ratio >= 10) return `\u00d7${formatSiNumber(ratio, 2)}`
  if (ratio > 0 && ratio <= 0.1) return `\u00f7${formatSiNumber(1 / ratio, 2)}`

  const sign = share > 0 ? '+' : '\u2212'
  return `${sign}${(Math.abs(share) * 100).toFixed(0)}%`
}

/** Which way a quantity moved, for anything that colours or points. */
export function directionOf(before: number, after: number): 'up' | 'down' | null {
  if (!Number.isFinite(before) || !Number.isFinite(after) || before === after) return null
  if (before !== 0 && Math.abs((after - before) / Math.abs(before)) < NOTICEABLE) return null
  return after > before ? 'up' : 'down'
}
