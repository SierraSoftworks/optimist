import { describe, expect, it } from 'vitest'
import { buildFermiFormula, fermiProvenance } from './fermiBuilder'

describe('Fermi decomposition builder', () => {
  it('turns three-point PERT estimates into a bounded probability product', () => {
    const formula = buildFermiFormula('product', [
      { name: 'adoption', low: 0.5, likely: 0.7, high: 0.9, unit: '' },
      { name: 'completion', low: 0.6, likely: 0.9, high: 1, unit: '' },
    ], 'probability')
    expect(formula).toMatchObject({
      type: 'bounded', lower: 0, upper: 1,
      input: {
        type: 'product',
        factors: [
          { type: 'literal', distribution: { type: 'scaled_beta' } },
          { type: 'literal', distribution: { type: 'scaled_beta' } },
        ],
      },
    })
    if (formula.type !== 'bounded' || formula.input.type !== 'product') throw new Error('unexpected formula')
    const distributions = formula.input.factors.map((factor) =>
      factor.type === 'literal' ? factor.distribution : null,
    )
    expect(distributions[0]?.alpha).toBeCloseTo(3)
    expect(distributions[0]?.beta).toBeCloseTo(3)
    expect(distributions[1]?.alpha).toBeCloseTo(4)
    expect(distributions[1]?.beta).toBeCloseTo(2)
  })

  it('preserves component units and requires ordered ranges', () => {
    expect(buildFermiFormula('ratio', [
      { name: 'work', low: 80, likely: 100, high: 140, unit: 'engineer_hours' },
      { name: 'people', low: 2, likely: 3, high: 4, unit: 'engineers' },
    ], 'non_negative')).toMatchObject({
      type: 'ratio',
      numerator: { unit: { engineer_hours: 1 } },
      denominator: { unit: { engineer: 1 } },
    })
    expect(() => buildFermiFormula('sum', [
      { name: 'invalid', low: 3, likely: 2, high: 1, unit: '' },
      { name: 'other', low: 1, likely: 1, high: 1, unit: '' },
    ], 'real')).toThrow('low ≤ likely ≤ high')
  })

  it('retains the elicitation in provenance', () => {
    expect(fermiProvenance('sum', [
      { name: 'design', low: 2, likely: 3, high: 5, unit: 'days' },
      { name: 'build', low: 4, likely: 6, high: 10, unit: 'days' },
    ], 2000)).toContain('design [2, 3, 5] days + build [4, 6, 10] days; 2000')
  })
})