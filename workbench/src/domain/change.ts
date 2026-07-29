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
 * The fold change at which a movement is worth a reader's attention.
 *
 * Measured as a multiple rather than as a share, because a share is not
 * symmetric: a doubling is +100% and the halving that undoes it is −50%, so any
 * threshold on the share alone calls one of them large and the other small. A
 * factor of two either way is the same event read from either end.
 *
 * Whether that event is welcome is not something this can know.
 */
export const NOTABLE_FOLD = 2

/**
 * How a proportional change reads.
 *
 * Always a signed share of the baseline, so that two quantities side by side can
 * be compared by eye. A mix of forms — a share here, a multiple there — makes a
 * reader work out which arithmetic each figure used before they can tell which
 * of the two moved further.
 *
 * Very large shares stay narrow by taking an SI prefix rather than a row of
 * digits: a success rate that went from a millionth to nearly one reads as
 * "+100M%", which is wide enough to say "off the scale" without being counted.
 *
 * A baseline of exactly zero has no proportional change at all — every increase
 * is infinitely many times it — so that case is named rather than divided by.
 */
export function describeChange(before: number, after: number): string | null {
  if (!Number.isFinite(before) || !Number.isFinite(after)) return null
  if (before === 0) {
    if (after === 0) return null
    return after > 0 ? 'from nothing' : 'to nothing'
  }

  const share = (after - before) / Math.abs(before)
  if (Math.abs(share) < NOTICEABLE) return null

  const percent = Math.abs(share) * 100
  const sign = share > 0 ? '+' : '\u2212'
  return `${sign}${percent >= 1000 ? formatSiNumber(percent, 2) : percent.toFixed(0)}%`
}

/**
 * Whether a movement is one to look at or one to note in passing.
 *
 * Drives how loudly a change is presented, in place of promoting it up a list.
 * Ordering by movement makes a list rearrange itself every time a variant is
 * picked, so the quantity somebody was reading is no longer where they left it.
 */
export function emphasisOf(before: number, after: number): 'notable' | 'slight' | null {
  if (!Number.isFinite(before) || !Number.isFinite(after) || before === after) return null
  if (before === 0) return after === 0 ? null : 'notable'
  if (Math.abs((after - before) / Math.abs(before)) < NOTICEABLE) return null

  // A quantity that crossed zero has changed in kind, not by a factor.
  const fold = after / before
  return fold <= 0 || fold >= NOTABLE_FOLD || fold <= 1 / NOTABLE_FOLD ? 'notable' : 'slight'
}

/** Which way a quantity moved, for anything that colours or points. */
export function directionOf(before: number, after: number): 'up' | 'down' | null {
  if (!Number.isFinite(before) || !Number.isFinite(after) || before === after) return null
  if (before !== 0 && Math.abs((after - before) / Math.abs(before)) < NOTICEABLE) return null
  return after > before ? 'up' : 'down'
}
