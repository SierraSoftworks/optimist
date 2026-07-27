import { describe, expect, it, vi } from 'vitest'
import { nextTick, ref } from 'vue'

import { useDraft } from './useDraft'

/** Lets a test hold a save open and decide when it finishes. */
function deferred<T = void>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((yes, no) => {
    resolve = yes
    reject = no
  })
  return { promise, resolve, reject }
}

describe('useDraft', () => {
  it('shows what is typed rather than what is stored', async () => {
    const stored = ref('900')
    const draft = useDraft(
      () => stored.value,
      async () => {},
    )

    draft.focus()
    draft.value.value = '900 * 2'
    expect(draft.dirty.value).toBe(true)
    // Nothing is sent until the field is left.
    expect(stored.value).toBe('900')
  })

  /**
   * The bug this exists to prevent.
   *
   * Somebody else's edit arrives on the change feed while a field is being
   * typed into. Taking it would replace the half-written expression with theirs.
   */
  it('does not overwrite a field being edited', async () => {
    const stored = ref('900')
    const draft = useDraft(
      () => stored.value,
      async () => {},
    )

    draft.focus()
    draft.value.value = 'half-typed'
    stored.value = 'someone else'
    await nextTick()

    expect(draft.value.value).toBe('half-typed')
  })

  it('takes the model while the field is idle', async () => {
    const stored = ref('900')
    const draft = useDraft(
      () => stored.value,
      async () => {},
    )

    stored.value = 'someone else'
    await nextTick()
    expect(draft.value.value).toBe('someone else')
  })

  it('saves on blur and reports that it did', async () => {
    const stored = ref('900')
    const save = vi.fn(async (value: string) => {
      stored.value = value
    })
    const draft = useDraft(() => stored.value, save)

    draft.focus()
    draft.value.value = '1200'
    await draft.blur()

    expect(save).toHaveBeenCalledWith('1200')
    expect(draft.state.value).toBe('saved')
  })

  it('says nothing when a field is entered and left unchanged', async () => {
    const save = vi.fn(async () => {})
    const draft = useDraft(() => '900', save)

    draft.focus()
    await draft.blur()

    expect(save).not.toHaveBeenCalled()
    expect(draft.state.value).toBe('clean')
  })

  /**
   * A rejected value has to stay on screen. Reverting it would throw away the
   * work and leave no way to see what was wrong with it.
   */
  it('keeps a rejected value and explains why', async () => {
    const stored = ref('900')
    const draft = useDraft(
      () => stored.value,
      async () => {
        throw new Error('that is not a number')
      },
    )

    draft.focus()
    draft.value.value = 'nonsense'
    await draft.blur()

    expect(draft.state.value).toBe('failed')
    expect(draft.error.value).toBe('that is not a number')
    expect(draft.value.value).toBe('nonsense')
  })

  it('does not let the model clobber a rejected edit', async () => {
    const stored = ref('900')
    const draft = useDraft(
      () => stored.value,
      async () => {
        throw new Error('no')
      },
    )

    draft.focus()
    draft.value.value = 'nonsense'
    await draft.blur()

    stored.value = 'something else'
    await nextTick()
    expect(draft.value.value).toBe('nonsense')
  })

  it('lets a rejected edit be thrown away deliberately', async () => {
    const stored = ref('900')
    const draft = useDraft(
      () => stored.value,
      async () => {
        throw new Error('no')
      },
    )

    draft.focus()
    draft.value.value = 'nonsense'
    await draft.blur()
    draft.revert()

    expect(draft.value.value).toBe('900')
    expect(draft.state.value).toBe('clean')
    expect(draft.error.value).toBeNull()
  })

  it('ignores the model while a save is in flight', async () => {
    const stored = ref('900')
    const gate = deferred()
    const draft = useDraft(
      () => stored.value,
      async () => {
        await gate.promise
      },
    )

    draft.focus()
    draft.value.value = '1200'
    const saving = draft.blur()

    stored.value = 'someone else'
    await nextTick()
    expect(draft.value.value).toBe('1200')

    gate.resolve()
    await saving
  })

  it('compares with the caller rule when values are not primitives', async () => {
    const stored = ref({ name: 'a', unit: 'op/s' })
    const save = vi.fn(async () => {})
    const draft = useDraft(() => stored.value, save, {
      equals: (a, b) => a.name === b.name && a.unit === b.unit,
    })

    draft.focus()
    // A different object holding the same values is not an edit.
    draft.value.value = { name: 'a', unit: 'op/s' }
    await draft.blur()
    expect(save).not.toHaveBeenCalled()

    draft.focus()
    draft.value.value = { name: 'a', unit: 'op' }
    await draft.blur()
    expect(save).toHaveBeenCalled()
  })
})
