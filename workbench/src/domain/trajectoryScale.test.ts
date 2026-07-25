import { describe, expect, it } from 'vitest'
import { normalize, percentile, symlog, trajectoryScale } from './trajectoryScale'

describe('percentile', () => {
  it('interpolates between order statistics', () => {
    expect(percentile([0, 10], 0.5)).toBe(5)
    expect(percentile([0, 1, 2, 3, 4], 0.1)).toBeCloseTo(0.4, 10)
    expect(percentile([0, 1, 2, 3, 4], 0.9)).toBeCloseTo(3.6, 10)
  })

  it('survives an empty series and out-of-range fractions', () => {
    expect(percentile([], 0.5)).toBe(0)
    expect(percentile([2, 4], -1)).toBe(2)
    expect(percentile([2, 4], 5)).toBe(4)
  })
})

describe('symlog', () => {
  it('keeps the sign and maps zero to zero', () => {
    expect(symlog(0, 1)).toBe(0)
    expect(symlog(-4, 1)).toBe(-symlog(4, 1))
  })

  /** A log axis cannot show a baseline at zero or a regression below it. */
  it('compresses magnitude so an outlier cannot dominate', () => {
    const near = symlog(1, 1)
    const far = symlog(100, 1)
    expect(far / near).toBeLessThan(10)
    expect(far).toBeGreaterThan(near)
  })

  /** Above the data range the transform is indistinguishable from linear. */
  it('degrades to linear as the threshold grows past the data', () => {
    expect(symlog(2, 1e6) / symlog(1, 1e6)).toBeCloseTo(2, 4)
  })
})

describe('trajectoryScale', () => {
  const samples = (means: number[], spreads: number[] = means.map(() => 0)) => ({
    means,
    lower: means.map((mean, index) => mean - spreads[index]!),
    upper: means.map((mean, index) => mean + spreads[index]!),
  })

  // A code yellow whose rebound period carries a standard deviation of 677%
  // around a mean of -87%, taken from the project this behaviour was built for.
  const rebound = {
    means: [0, 0, 0, 0.006, 0.171, 0.112, 0.103, -0.866, 0.016, 0.041, 0.038, 0.038, 0.038],
    spreads: [0, 0, 0, 0.018, 0.438, 0.301, 0.267, 6.766, 0.648, 0.112, 0.101, 0.101, 0.101],
  }

  it('spans the whole series when no period is unusually uncertain', () => {
    const scale = trajectoryScale(samples([-0.1, 0, 0.1, 0.2]))
    expect(scale.clipped).toBe(false)
    expect(scale.lower).toBeLessThan(-0.1)
    expect(scale.upper).toBeGreaterThan(0.2)
  })

  /** Sizing the axis to that band flattens every other period onto zero. */
  it('cuts back a band that dwarfs its own p10-p90 extent', () => {
    const scale = trajectoryScale(samples(rebound.means, rebound.spreads))
    expect(scale.clipped).toBe(true)
    expect(scale.lower).toBeGreaterThan(-7.63)
    expect(scale.upper).toBeLessThan(5.9)
  })

  /**
   * A mean is the signal. Clipping one would hide the very regression the chart
   * exists to show, so every mean must fit even though the band around it is
   * wide enough to trigger the clip.
   */
  it('never clips a mean, however wide the band around it', () => {
    const scale = trajectoryScale(samples(rebound.means, rebound.spreads))
    expect(scale.clipped).toBe(true)
    for (const mean of rebound.means) {
      expect(mean).toBeGreaterThanOrEqual(scale.lower)
      expect(mean).toBeLessThanOrEqual(scale.upper)
    }
    expect(normalize(-0.866, scale)).toBeGreaterThanOrEqual(0)
  })

  /** A steadily improving plan is signal throughout and must not be trimmed. */
  it('keeps every period of a monotonic trend in frame', () => {
    const means = Array.from({ length: 13 }, (_, index) => 0.05 * index)
    const scale = trajectoryScale(samples(means))
    expect(scale.upper).toBeGreaterThanOrEqual(0.6)
    expect(normalize(0.6, scale)).toBeLessThanOrEqual(1)
  })

  it('always keeps the baseline in view', () => {
    expect(trajectoryScale(samples([4, 5, 6])).lower).toBeLessThanOrEqual(0)
    expect(trajectoryScale(samples([-6, -5, -4])).upper).toBeGreaterThanOrEqual(0)
  })

  it('produces a usable axis from an empty or degenerate series', () => {
    const degenerate = [
      { means: [], lower: [], upper: [] },
      samples([0, 0, 0]),
      { means: [Number.NaN], lower: [Number.POSITIVE_INFINITY], upper: [Number.NaN] },
    ]
    for (const input of degenerate) {
      const scale = trajectoryScale(input)
      expect(scale.linthresh).toBeGreaterThan(0)
      expect(scale.upper).toBeGreaterThan(scale.lower)
      expect(Number.isFinite(normalize(0, scale))).toBe(true)
    }
  })
})

describe('normalize', () => {
  const flat = (means: number[]) => ({ means, lower: means, upper: means })

  it('puts the axis floor at zero and its ceiling at one', () => {
    const scale = trajectoryScale(flat([-0.2, 0, 0.2]))
    expect(normalize(scale.lower, scale)).toBeCloseTo(0, 10)
    expect(normalize(scale.upper, scale)).toBeCloseTo(1, 10)
  })

  /** Clipped band edges must leave the frame rather than pile up on it. */
  it('returns positions outside the unit range for clipped band edges', () => {
    const means = Array.from({ length: 12 }, (_, index) => 0.01 * index)
    const scale = trajectoryScale({
      means,
      lower: means,
      upper: means.map((mean, index) => (index === 6 ? mean + 9 : mean + 0.01)),
    })
    expect(scale.clipped).toBe(true)
    expect(normalize(9, scale)).toBeGreaterThan(1)
  })

  it('preserves order', () => {
    const scale = trajectoryScale(flat([-1, 0, 1]))
    expect(normalize(-0.5, scale)).toBeLessThan(normalize(0, scale))
    expect(normalize(0, scale)).toBeLessThan(normalize(0.5, scale))
  })
})
