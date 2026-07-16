import { describe, expect, it } from 'vitest'
import { compileFermiEquation, FermiEquationError } from './fermiEquation'
import type { FermiComponentDraft } from './fermiBuilder'
import { divideUnits, formatUnitExpression } from './unitExpression'

const pianoVariables: FermiComponentDraft[] = [
  variable('people', 1_500_000, 'people'),
  variable('people_per_household', 3, 'people/household'),
  variable('households_per_piano', 20, 'households/piano'),
  variable('days_per_tuning', 180, 'days/tuning'),
  variable('pianos_per_tuning', 1, 'pianos/tuning'),
]

describe('Fermi equations', () => {
  it('computes the piano central estimate and identifies its residual dimension', () => {
    const compiled = compileFermiEquation(
      'people / people_per_household / households_per_piano / days_per_tuning * pianos_per_tuning',
      pianoVariables,
      'non_negative',
    )
    expect(compiled.central).toBeCloseTo(138.8888889)
    expect(formatUnitExpression(compiled.unit)).toBe('piano^2/day')
    expect(formatUnitExpression(divideUnits(compiled.unit, { piano: 1, day: -1 }))).toBe('piano')
  })

  it('resolves the piano goal when tuning interval includes its subject dimension', () => {
    const variables = pianoVariables.map((value) =>
      value.name === 'days_per_tuning' ? { ...value, unit: 'piano*days/tuning' } : value,
    )
    const compiled = compileFermiEquation(
      'people / people_per_household / households_per_piano / days_per_tuning * pianos_per_tuning',
      variables,
      'non_negative',
    )
    expect(formatUnitExpression(compiled.unit)).toBe('piano/day')
  })

  it('supports grouped sums and reports unknown variables and additive mismatches', () => {
    const values = [variable('x', 2, 'day'), variable('y', 3, 'day'), variable('z', 4, 'piano')]
    expect(compileFermiEquation('(x + y) * 2', values, 'real').central).toBe(10)
    expect(() => compileFermiEquation('x + missing', values, 'real')).toThrow(FermiEquationError)
    expect(() => compileFermiEquation('x + z', values, 'real')).toThrow('matching units')
  })
})

function variable(name: string, likely: number, unit: string): FermiComponentDraft {
  return { name, likely, low: likely / 10, high: likely * 10, unit, mode: 'order_of_magnitude' }
}