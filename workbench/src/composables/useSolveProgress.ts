import { computed, onScopeDispose, ref, toValue, watch, type MaybeRefOrGetter } from 'vue'

import { expected, progress, remaining, remember } from '../domain/solveEstimate'

/** How often the elapsed clock is read while a solve is in flight. */
const TICK_MS = 100

/**
 * Watches a solve and reports how far through it appears to be.
 *
 * Timing happens here rather than in the query layer because what a reader wants
 * measured is the wait — from asking to seeing — which starts before the request
 * is issued and ends after the answer has been handed over. The query only knows
 * about the middle of that.
 *
 * The `shape` is what makes two solves comparably expensive. Passing a shape
 * that includes the variant would mean every variant had to be solved once
 * before any of them could be predicted, and they all cost the same.
 */
export function useSolveProgress(
  active: MaybeRefOrGetter<boolean>,
  shape: MaybeRefOrGetter<string>,
) {
  const elapsed = ref(0)
  const started = ref<number | null>(null)
  const estimate = ref<number | null>(null)
  let ticker: ReturnType<typeof setInterval> | null = null

  const stop = () => {
    if (ticker !== null) clearInterval(ticker)
    ticker = null
  }

  watch(
    () => toValue(active),
    (running) => {
      if (running) {
        if (started.value !== null) return
        started.value = performance.now()
        elapsed.value = 0
        estimate.value = expected(toValue(shape))
        ticker = setInterval(() => {
          if (started.value !== null) elapsed.value = performance.now() - started.value
        }, TICK_MS)
        return
      }
      stop()
      if (started.value === null) return
      remember(toValue(shape), performance.now() - started.value)
      started.value = null
      elapsed.value = 0
    },
    { immediate: true },
  )
  onScopeDispose(stop)

  /**
   * How complete the solve looks, or null while nothing is known about the cost.
   *
   * A first solve of a given shape has nothing to predict from, and inventing a
   * curve for it would be theatre. Reporting null lets the indicator say "working"
   * honestly until there is something to say instead.
   */
  const fraction = computed(() =>
    estimate.value === null ? null : progress(elapsed.value, estimate.value),
  )

  const secondsLeft = computed(() =>
    estimate.value === null ? null : remaining(elapsed.value, estimate.value),
  )

  /** What the wait should be called, once it has gone on long enough to name. */
  const caption = computed(() => {
    if (secondsLeft.value === null) {
      return estimate.value === null ? 'solving' : 'nearly there'
    }
    return secondsLeft.value < 1
      ? 'about a second left'
      : `about ${Math.ceil(secondsLeft.value)}s left`
  })

  return { elapsed, fraction, secondsLeft, caption }
}
