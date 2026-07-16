import { describe, expect, it } from 'vitest'
import { distributionPreview } from './distributionPreview'

describe('distribution preview', () => {
  it('represents point estimates as exact markers without inventing a spread', () => {
    const preview = distributionPreview({ type: 'point', value: 0.7 }, [0, 1])
    expect(preview.marker).toBe(0.7)
    expect(preview.domain).toEqual([0, 1])
    expect(preview.density).toEqual([])
    expect(preview.summary).toContain('No uncertainty')
  })

  it('renders a symmetric Normal curve centered on its mean', () => {
    const preview = distributionPreview({ type: 'normal', mean: 10, standard_deviation: 2 })
    expect(preview.domain).toEqual([2, 18])
    expect(preview.density[0]!.density).toBeCloseTo(preview.density.at(-1)!.density)
    expect(preview.density[60]!.density).toBeCloseTo(1)
    expect(preview.summary).toContain('6 to 14')
  })

  it('renders LogNormal support above zero with a right tail', () => {
    const preview = distributionPreview({ type: 'log_normal', location: 0, scale: 0.5 })
    expect(preview.domain[0]).toBeGreaterThan(0)
    expect(preview.domain[1]).toBeGreaterThan(1)
    expect(preview.summary).toContain('long upper tail')
    expect(preview.density.every((point) => Number.isFinite(point.density))).toBe(true)
  })

  it('keeps Beta and Scaled Beta samples inside hard support', () => {
    const beta = distributionPreview({ type: 'beta', alpha: 8, beta: 2 })
    const scaled = distributionPreview({
      type: 'scaled_beta', alpha: 2, beta: 5, lower: -1, upper: 1,
    })
    expect(beta.domain).toEqual([0, 1])
    expect(beta.summary).toContain('Mean 0.8')
    expect(scaled.domain).toEqual([-1, 1])
    expect(scaled.density.every((point) => point.value >= -1 && point.value <= 1)).toBe(true)
  })

  it('remains finite while form parameters are temporarily invalid', () => {
    for (const distribution of [
      { type: 'normal' as const, mean: Number.NaN, standard_deviation: 0 },
      { type: 'log_normal' as const, location: Number.NaN, scale: -1 },
      { type: 'beta' as const, alpha: 0, beta: Number.NaN },
      { type: 'scaled_beta' as const, alpha: -1, beta: 0, lower: 2, upper: 1 },
    ]) {
      const preview = distributionPreview(distribution)
      expect(preview.domain.every(Number.isFinite)).toBe(true)
      expect(preview.density.every((point) => Number.isFinite(point.density))).toBe(true)
    }
  })
})
