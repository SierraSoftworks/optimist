import { describe, expect, it } from 'vitest'

import {
  activationSeries,
  effectProfileInput,
  effectProfileValid,
  emptyEffectProfileForm,
  type EffectProfileForm,
} from './effectProfile'

function form(overrides: Partial<EffectProfileForm> = {}): EffectProfileForm {
  return { ...emptyEffectProfileForm(), enabled: true, ...overrides }
}

describe('effectProfileInput', () => {
  it('returns null for a permanent effect', () => {
    expect(effectProfileInput(emptyEffectProfileForm())).toBeNull()
  })

  it('omits an absent ramp and aftereffect', () => {
    const input = effectProfileInput(form({ hold: 2 }))
    expect(input).toEqual({
      ramp: null,
      hold: { source: 'pointMass(2)', seed: 42, sample_count: 256, target_unit: { duration: 1 } },
      release: { type: 'immediate' },
      aftereffect: null,
    })
  })

  it('carries the rebound magnitude as a dimensionless multiplier', () => {
    const input = effectProfileInput(
      form({ hold: 2, reboundEnabled: true, reboundMagnitude: 1.25, reboundHold: 1 }),
    )
    expect(input?.aftereffect?.magnitude).toEqual({
      source: 'pointMass(1.25)',
      seed: 42,
      sample_count: 256,
      target_unit: {},
    })
    expect(input?.aftereffect?.hold?.source).toBe('pointMass(1)')
  })

  it('selects the release span field matching the chosen shape', () => {
    expect(effectProfileInput(form({ release: 'linear', releaseSpan: 3 }))?.release).toEqual({
      type: 'linear',
      over: { source: 'pointMass(3)', seed: 42, sample_count: 256, target_unit: { duration: 1 } },
    })
    expect(
      effectProfileInput(form({ release: 'exponential', releaseSpan: 2 }))?.release,
    ).toEqual({
      type: 'exponential',
      half_life: {
        source: 'pointMass(2)',
        seed: 42,
        sample_count: 256,
        target_unit: { duration: 1 },
      },
    })
  })
})

describe('effectProfileValid', () => {
  it('accepts a permanent effect and rejects a shape that never departs from one', () => {
    expect(effectProfileValid(emptyEffectProfileForm())).toBe(true)
    expect(effectProfileValid(form({ ramp: 0, hold: 0, reboundEnabled: false }))).toBe(false)
  })

  it('requires a positive span for a gradual release', () => {
    expect(effectProfileValid(form({ release: 'linear', releaseSpan: 0 }))).toBe(false)
    expect(effectProfileValid(form({ release: 'linear', releaseSpan: 1 }))).toBe(true)
  })

  it('rejects negative or non-finite durations', () => {
    expect(effectProfileValid(form({ hold: -1 }))).toBe(false)
    expect(effectProfileValid(form({ ramp: Number.NaN }))).toBe(false)
    expect(effectProfileValid(form({ reboundEnabled: true, reboundMagnitude: Number.NaN }))).toBe(
      false,
    )
  })
})

describe('activationSeries', () => {
  it('holds at full strength for a permanent effect', () => {
    const { activation, rebound } = activationSeries(emptyEffectProfileForm(), 4)
    expect(activation).toEqual([1, 1, 1, 1])
    expect(rebound).toEqual([0, 0, 0, 0])
  })

  it('produces a rectangular pulse for a bounded hold', () => {
    expect(activationSeries(form({ hold: 2 }), 4).activation).toEqual([1, 1, 0, 0])
  })

  it('rises linearly across the ramp', () => {
    const { activation } = activationSeries(form({ ramp: 2, hold: 1 }), 4)
    expect(activation[0]).toBeCloseTo(1 / 3, 12)
    expect(activation[1]).toBeCloseTo(2 / 3, 12)
    expect(activation[2]).toBe(1)
    expect(activation[3]).toBe(0)
  })

  it('declines to zero across a linear release', () => {
    const { activation } = activationSeries(
      form({ hold: 1, release: 'linear', releaseSpan: 3 }),
      5,
    )
    expect(activation[0]).toBe(1)
    expect(activation[1]).toBeCloseTo(0.75, 12)
    expect(activation[2]).toBeCloseTo(0.5, 12)
    expect(activation[3]).toBeCloseTo(0.25, 12)
    expect(activation[4]).toBe(0)
  })

  it('halves across an exponential half-life', () => {
    const { activation } = activationSeries(
      form({ hold: 1, release: 'exponential', releaseSpan: 2 }),
      4,
    )
    expect(activation[1]).toBeCloseTo(2 ** -0.5, 12)
    expect(activation[3]).toBeCloseTo(activation[1] / 2, 12)
  })

  it('fires the rebound only after the primary effect releases', () => {
    const { activation, rebound } = activationSeries(
      form({ hold: 2, reboundEnabled: true, reboundHold: 1 }),
      4,
    )
    expect(activation).toEqual([1, 1, 0, 0])
    expect(rebound).toEqual([0, 0, 1, 0])
  })
})
