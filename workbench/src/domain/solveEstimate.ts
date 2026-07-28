/**
 * How long a solve is likely to take, learned from the ones already done.
 *
 * The server does not report progress: a relaxation does not know how many
 * passes it needs until it has taken them, so any figure it sent partway through
 * would be a guess dressed as a measurement. What is genuinely known is how long
 * the same question took last time, and a design being reviewed is asked the
 * same question repeatedly — a variant switched, a control nudged, an edit made
 * and re-solved.
 *
 * Estimates are therefore held per *shape*: the controls that decide how much
 * arithmetic there is, without the ones that only decide which answer comes out.
 * Two variants of one design cost the same to solve, so the first teaches the
 * second.
 */

/**
 * Weight given to the newest measurement.
 *
 * High enough that adding a component or lengthening the horizon is reflected
 * within a couple of solves, low enough that one request delayed behind another
 * does not become the expectation. This is an exponentially weighted mean, so
 * the effective window is about 1/α — here, the last five or so solves.
 */
const WEIGHT = 0.2

/**
 * Shapes remembered before the oldest is dropped.
 *
 * A shape exists per design and per combination of sampling controls, so a
 * session touches a handful. The bound is here because nothing else would ever
 * remove one.
 */
const CAPACITY = 64

/** Measurements above this are treated as an interrupted request, not a solve. */
const IMPLAUSIBLE_MS = 10 * 60 * 1000

const durations = new Map<string, number>()

/**
 * Records how long one solve took.
 *
 * Ignores implausible measurements, which a tab suspended midway through a
 * request would otherwise contribute: the browser reports the wall clock, not
 * the time the machine spent, and a laptop lid closed for an hour would
 * otherwise teach the estimator to promise an hour.
 */
export function remember(shape: string, milliseconds: number): void {
  if (!Number.isFinite(milliseconds) || milliseconds <= 0) return
  if (milliseconds > IMPLAUSIBLE_MS) return

  const previous = durations.get(shape)
  const next = previous === undefined ? milliseconds : previous + WEIGHT * (milliseconds - previous)
  // Re-inserting moves the key to the end of the map's iteration order, which is
  // what makes the eviction below drop the least recently measured shape.
  durations.delete(shape)
  durations.set(shape, next)

  if (durations.size > CAPACITY) {
    const oldest = durations.keys().next()
    if (!oldest.done) durations.delete(oldest.value)
  }
}

/** How long a solve of this shape is expected to take, if it is known at all. */
export function expected(shape: string): number | null {
  return durations.get(shape) ?? null
}

/**
 * How far through a solve of this shape we believe we are.
 *
 * The curve is deliberately incomplete. Reaching a hundred per cent before the
 * answer arrives would be a lie the reader can see through, and one that makes
 * every subsequent estimate less believable, so the bar covers nine tenths of
 * the bar in the expected time and then approaches — without reaching — the end
 * for as long as the solve overruns.
 */
export function progress(elapsed: number, estimate: number): number {
  if (estimate <= 0) return 0
  if (elapsed < estimate) return 0.9 * (elapsed / estimate)
  return 0.9 + 0.09 * (1 - Math.exp(-(elapsed - estimate) / estimate))
}

/** Seconds still expected, or null once the solve has outrun its estimate. */
export function remaining(elapsed: number, estimate: number): number | null {
  const left = estimate - elapsed
  return left > 0 ? left / 1000 : null
}

/** Discards every measurement, so one test cannot teach the next. */
export function forget(): void {
  durations.clear()
}
