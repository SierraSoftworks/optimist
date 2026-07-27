import { describe, expect, it } from 'vitest'
import { formatSiNumber } from './humanNumber'

describe('formatSiNumber', () => {
  it('carries magnitude in the prefix rather than in digits', () => {
    expect(formatSiNumber(1_370_000)).toBe('1.37M')
    expect(formatSiNumber(721)).toBe('721')
    expect(formatSiNumber(0.000137)).toBe('137µ')
  })

  /**
   * SI rather than the financial scale used when authoring estimates: a reader of
   * a measured quantity expects `k` and `G`, not `K` and `B`.
   */
  it('follows SI rather than the financial scale', () => {
    expect(formatSiNumber(2_500)).toBe('2.5k')
    expect(formatSiNumber(4_000_000_000)).toBe('4G')
  })

  it('keeps every label short enough for an axis gutter', () => {
    for (const value of [1.77e-13, 0.02, 86.8, 193, 721, 1e9, 1e12]) {
      expect(formatSiNumber(value).length).toBeLessThanOrEqual(6)
    }
  })

  it('writes zero and signs plainly', () => {
    expect(formatSiNumber(0)).toBe('0')
    expect(formatSiNumber(-0.0025)).toBe('-2.5m')
  })

  it('falls back to exponent form below the smallest prefix', () => {
    expect(formatSiNumber(1e-20)).toBe('1.0e-20')
  })

  it('reports a value it cannot render', () => {
    expect(formatSiNumber(Number.NaN)).toBe('—')
    expect(formatSiNumber(Number.POSITIVE_INFINITY)).toBe('—')
  })
})
