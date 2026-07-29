import { describe, expect, it } from 'vitest'

import { densityProfile, kernelDensity, quantileOf } from './density'

/**
 * A deterministic normal sample.
 *
 * Box–Muller over mulberry32. A plain linear congruential generator is not good
 * enough here: consecutive LCG outputs lie on lattice planes, and Box–Muller
 * consumes two at a time, so the resulting sample carries visible lumps that a
 * density estimator is right to report. Testing a mode detector against a
 * generator with structure of its own would measure the generator.
 */
function normals(count: number, mean: number, deviation: number, seed: number): number[] {
  let state = seed >>> 0
  const next = () => {
    state = (state + 0x6d2b79f5) >>> 0
    let t = Math.imul(state ^ (state >>> 15), 1 | state)
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
  return Array.from({ length: count }, () => {
    const radius = Math.sqrt(-2 * Math.log(next() || Number.MIN_VALUE))
    return mean + deviation * radius * Math.cos(2 * Math.PI * next())
  })
}

describe('quantileOf', () => {
  it('interpolates between order statistics', () => {
    const sorted = [0, 1, 2, 3, 4]
    expect(quantileOf(sorted, 0)).toBe(0)
    expect(quantileOf(sorted, 1)).toBe(4)
    expect(quantileOf(sorted, 0.5)).toBe(2)
    expect(quantileOf(sorted, 0.25)).toBe(1)
  })

  it('survives a sample too short to interpolate within', () => {
    expect(quantileOf([7], 0.9)).toBe(7)
    expect(Number.isNaN(quantileOf([], 0.5))).toBe(true)
  })
})

describe('kernelDensity', () => {
  it('declines to estimate what has no spread', () => {
    expect(kernelDensity([])).toBeNull()
    expect(kernelDensity([3])).toBeNull()
    // A quantity pinned at a limit in every draw. There is nothing to smooth,
    // and the caller is expected to draw it as a point.
    expect(kernelDensity(Array.from({ length: 200 }, () => 2))).toBeNull()
  })

  it('reports one mode for a sample that has one', () => {
    const density = kernelDensity(normals(600, 10, 2, 1))
    expect(density).not.toBeNull()
    expect(density!.modes).toBe(1)
  })

  /**
   * The reason this module exists.
   *
   * Two well-separated components, in the proportions a design near a fold
   * produces. The rule-of-thumb bandwidth merges them; the estimator has to find
   * a smaller one and report both.
   */
  it('finds both modes of a separated mixture', () => {
    const draws = [...normals(400, 0, 1, 7), ...normals(200, 12, 1, 99)]
    const density = kernelDensity(draws)
    expect(density).not.toBeNull()
    expect(density!.modes).toBeGreaterThanOrEqual(2)
  })

  /**
   * A minority branch is the case that matters most and is easiest to lose.
   *
   * When a tenth of draws have collapsed, the second mode is short. Judging it
   * against the tallest peak would discard it as a wobble, which is why
   * prominence is measured against the shorter of the two peaks instead.
   */
  it('finds a mode that only a small share of draws sit in', () => {
    const draws = [...normals(540, 0, 1, 41), ...normals(60, 10, 1, 43)]
    const density = kernelDensity(draws)
    expect(density!.modes).toBeGreaterThanOrEqual(2)
  })

  /**
   * The search descends through narrower bandwidths, and at a narrow enough one
   * any sample looks lumpy. Reporting a second mode for a plainly unimodal
   * sample would make the chart cry wolf on every design.
   */
  it('does not manufacture a mode from sampling noise', () => {
    for (const seed of [2, 17, 53, 101, 997]) {
      const density = kernelDensity(normals(500, 0, 1, seed))
      expect(density!.modes, `seed ${seed}`).toBe(1)
    }
  })

  it('integrates to approximately one', () => {
    const density = kernelDensity(normals(800, 0, 1, 21))!
    const step = density.x[1] - density.x[0]
    const mass = density.y.reduce((total, value) => total + value * step, 0)
    expect(mass).toBeGreaterThan(0.95)
    expect(mass).toBeLessThan(1.05)
  })

  it('puts its peak near the centre of a symmetric sample', () => {
    const density = kernelDensity(normals(800, 25, 3, 5))!
    const peak = density.x[density.y.indexOf(Math.max(...density.y))]
    expect(Math.abs(peak - 25)).toBeLessThan(1)
  })

  it('never returns a negative density', () => {
    const density = kernelDensity(normals(300, 0, 1, 31))!
    expect(density.y.every((value) => value >= 0)).toBe(true)
  })
})

describe('densityProfile', () => {
  /** Where the profile peaks, as a value on the grid it was asked for. */
  function peakOf(profile: number[], low: number, high: number): number {
    const width = (high - low) / profile.length
    return low + (profile.indexOf(Math.max(...profile)) + 0.5) * width
  }

  it('declines to estimate what it cannot', () => {
    expect(densityProfile([], 0, 1, 40)).toBeNull()
    expect(densityProfile([0.5], 1, 0, 40)).toBeNull()
    expect(densityProfile([0.5], 0, 1, 1)).toBeNull()
  })

  it('scales its tallest cell to one', () => {
    const profile = densityProfile(normals(600, 5, 1, 3), 0, 10, 40)!
    expect(Math.max(...profile)).toBeCloseTo(1, 10)
    expect(profile.every((value) => value >= 0)).toBe(true)
  })

  it('peaks where the sample is', () => {
    const profile = densityProfile(normals(600, 7, 0.5, 4), 0, 10, 40)!
    expect(Math.abs(peakOf(profile, 0, 10) - 7)).toBeLessThan(0.5)
  })

  it('leaves a valley between two branches', () => {
    const draws = [...normals(400, 1, 0.3, 5), ...normals(400, 9, 0.3, 6)]
    const profile = densityProfile(draws, 0, 10, 40)!
    const middle = profile.slice(16, 24)
    expect(Math.max(...middle)).toBeLessThan(0.1)
    expect(Math.max(...profile.slice(0, 8))).toBeGreaterThan(0.5)
    expect(Math.max(...profile.slice(32))).toBeGreaterThan(0.5)
  })

  it('keeps a minority branch visible', () => {
    const draws = [...normals(900, 1, 0.3, 7), ...normals(100, 9, 0.3, 8)]
    const profile = densityProfile(draws, 0, 10, 40)!
    expect(Math.max(...profile.slice(32))).toBeGreaterThan(0.05)
  })

  it('draws a sample with no spread as a cell rather than nothing', () => {
    const profile = densityProfile(Array.from({ length: 200 }, () => 4), 0, 10, 40)!
    expect(Math.max(...profile)).toBeCloseTo(1, 10)
    expect(Math.abs(peakOf(profile, 0, 10) - 4)).toBeLessThan(0.5)
  })

  it('ignores draws outside the grid rather than folding them in', () => {
    const profile = densityProfile([-100, 5, 5, 5, 200], 0, 10, 40)!
    expect(Math.abs(peakOf(profile, 0, 10) - 5)).toBeLessThan(0.5)
  })
})
