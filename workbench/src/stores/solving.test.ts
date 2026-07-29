import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it } from 'vitest'

import type { RunningSolve } from '../api/types'
import { useSolvingStore } from './solving'

beforeEach(() => setActivePinia(createPinia()))

function solve(overrides: Partial<RunningSolve> = {}): RunningSolve {
  return {
    kind: 'analysis',
    variant: null,
    sequence: 3,
    fraction: 0.4,
    step: 2,
    steps: 20,
    pass: 118,
    ...overrides,
  }
}

describe('the solving store', () => {
  it('takes the list the socket opens with as the truth', () => {
    const solving = useSolvingStore()
    solving.update('checkout', solve({ variant: 'stale' }))
    solving.replace('checkout', [solve({ variant: 'warm-cache' })])

    expect(solving.variant('checkout', 'stale')).toBeNull()
    expect(solving.variant('checkout', 'warm-cache')).not.toBeNull()
  })

  it('clears a solve when it finishes', () => {
    const solving = useSolvingStore()
    solving.update('checkout', solve())
    solving.finish('checkout', { kind: 'analysis', variant: null, sequence: 3 })

    expect(solving.variant('checkout', null)).toBeNull()
  })

  /**
   * Two people asking one question with different sample counts are two solves
   * under one name, and their frames interleave.
   */
  it('keeps whichever solve of a variant has got furthest', () => {
    const solving = useSolvingStore()
    solving.update('checkout', solve({ fraction: 0.6 }))
    solving.update('checkout', solve({ fraction: 0.2 }))

    expect(solving.variant('checkout', null)?.fraction).toBe(0.6)
  })

  /** An edit lands and the answer starts again; the indicator should follow it. */
  it('follows a solve of a later version of the design backwards', () => {
    const solving = useSolvingStore()
    solving.update('checkout', solve({ fraction: 0.9 }))
    solving.update('checkout', solve({ fraction: 0.1, sequence: 4 }))

    expect(solving.variant('checkout', null)?.fraction).toBe(0.1)
  })

  /** A variant being weighed is still that variant, as far as its row goes. */
  it('reports a comparison against the variant it is about', () => {
    const solving = useSolvingStore()
    solving.update('checkout', solve({ kind: 'comparison', variant: 'warm-cache' }))

    expect(solving.variant('checkout', 'warm-cache')?.kind).toBe('comparison')
  })

  it('keeps one design\'s solves out of another\'s', () => {
    const solving = useSolvingStore()
    solving.update('checkout', solve())

    expect(solving.variant('billing', null)).toBeNull()
    expect(Object.keys(solving.solves('checkout'))).toHaveLength(1)
  })

  /** A bar turning over a socket that has dropped is a lie. */
  it('forgets everything when the feed goes away', () => {
    const solving = useSolvingStore()
    solving.update('checkout', solve())
    solving.forget('checkout')

    expect(solving.solves('checkout')).toEqual({})
  })
})
