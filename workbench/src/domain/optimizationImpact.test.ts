import { describe, expect, it } from 'vitest'
import { impactTone, relativeImprovement } from './optimizationImpact'

describe('relative optimization impact', () => {
  it('normalizes direction-oriented improvement by baseline magnitude', () => {
    expect(relativeImprovement(0.12, 0.5)).toBeCloseTo(0.24)
    expect(relativeImprovement(-0.12, 0.5)).toBeCloseTo(-0.24)
    expect(relativeImprovement(0.12, -0.5)).toBeCloseTo(0.24)
  })

  it('does not define a percentage against a zero or absent baseline', () => {
    expect(relativeImprovement(0.12, 0)).toBeNull()
    expect(relativeImprovement(null, 0.5)).toBeNull()
  })
})

describe('optimization impact colors', () => {
  it('treats increases as good only for maximize objectives', () => {
    expect(impactTone(0.2, 'maximize')).toBe('positive')
    expect(impactTone(0.2, 'minimize')).toBe('negative')
  })

  it('treats decreases as good only for minimize objectives', () => {
    expect(impactTone(-0.2, 'minimize')).toBe('positive')
    expect(impactTone(-0.2, 'maximize')).toBe('negative')
  })

  it('keeps absent and zero shifts neutral', () => {
    expect(impactTone(0, 'maximize')).toBe('neutral')
    expect(impactTone(null, 'minimize')).toBe('neutral')
  })
})