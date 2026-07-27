import { computed, ref, watch, type Ref } from 'vue'

import { ApiError } from '../api/client'

/** Where a field is in its journey from typed to saved. */
export type SaveState = 'clean' | 'editing' | 'saving' | 'saved' | 'failed'

export interface Draft<T> {
  /** What the field shows. Bind this, not the model. */
  value: Ref<T>
  state: Ref<SaveState>
  /** Why the last save failed, for showing against the field. */
  error: Ref<string | null>
  /** Advice the server offered alongside the failure. */
  advice: Ref<string[]>
  dirty: Ref<boolean>
  focus: () => void
  /** Commits if anything changed. Safe to call on blur and on Enter both. */
  blur: () => void
  /** Throws the local edit away and shows the model again. */
  revert: () => void
}

/**
 * A field that is edited locally and saved when it is left.
 *
 * # Why the model is not bound directly
 *
 * Binding an input straight to server state looks simplest and produces a field
 * that cannot be typed into. Every keystroke would have to round-trip before it
 * appeared, and any save that failed — or any unrelated edit arriving on the
 * change feed — would overwrite the half-finished expression with the old one.
 * That is not a hypothetical: it is what the first version of this workbench did,
 * and it made properties look read-only whenever another property was invalid.
 *
 * So the field owns what it shows. The model is copied in only when the field is
 * not being used, which is what lets somebody else's edit appear without
 * discarding a sentence being typed here.
 *
 * # Why saving happens on blur
 *
 * Saving per keystroke sends a stream of states nobody meant to keep, and each
 * one is a solve on the server. Saving only on Enter loses work whenever
 * somebody types a value and clicks elsewhere, which is most of the time.
 * Leaving the field is the moment a person considers the value finished.
 */
export function useDraft<T>(
  source: () => T,
  save: (value: T) => Promise<unknown>,
  options: { equals?: (a: T, b: T) => boolean } = {},
): Draft<T> {
  const equals = options.equals ?? ((a, b) => a === b)

  const value = ref(source()) as Ref<T>
  const state = ref<SaveState>('clean')
  const error = ref<string | null>(null)
  const advice = ref<string[]>([])
  const focused = ref(false)

  const dirty = computed(() => !equals(value.value, source())) as Ref<boolean>

  // Take the model's value only when the field is idle. While it is focused or
  // holds an unsaved edit, what is on screen is the more recent truth.
  watch(source, (next) => {
    if (focused.value || state.value === 'saving') return
    if (state.value === 'failed' && dirty.value) return
    value.value = next
  })

  function focus() {
    focused.value = true
    if (state.value === 'saved') state.value = 'clean'
  }

  async function blur() {
    focused.value = false
    if (!dirty.value) {
      // Nothing changed, so there is nothing to report. A field that flashed
      // "saved" for merely being clicked into would make the indicator noise.
      if (state.value !== 'failed') state.value = 'clean'
      return
    }
    const attempt = value.value
    state.value = 'saving'
    error.value = null
    advice.value = []
    try {
      await save(attempt)
      state.value = 'saved'
    } catch (failure) {
      state.value = 'failed'
      error.value = failure instanceof Error ? failure.message : String(failure)
      advice.value = failure instanceof ApiError ? failure.advice : []
    }
  }

  function revert() {
    value.value = source()
    state.value = 'clean'
    error.value = null
    advice.value = []
  }

  return { value, state, error, advice, dirty, focus, blur, revert }
}
