import { describe, expect, it } from 'vitest'
import {
  divideUnits,
  formatUnitExpression,
  multiplyUnits,
  parseUnitExpression,
  unitsEqual,
} from './unitExpression'

describe('human unit expressions', () => {
  it('parses ratios, products, powers, and singular/plural forms', () => {
    expect(parseUnitExpression('people / household')).toEqual({ person: 1, household: -1 })
    expect(parseUnitExpression('households/piano')).toEqual({ household: 1, piano: -1 })
    expect(parseUnitExpression('(pianos / day)^2')).toEqual({ piano: 2, day: -2 })
    expect(formatUnitExpression({ piano: 1, day: -1 })).toBe('piano/day')
  })

  it('composes the entered piano variables and exposes the dimensional mismatch', () => {
    let result = parseUnitExpression('people')
    result = divideUnits(result, parseUnitExpression('people/household'))
    result = divideUnits(result, parseUnitExpression('households/piano'))
    result = divideUnits(result, parseUnitExpression('days/tuning'))
    result = multiplyUnits(result, parseUnitExpression('pianos/tuning'))

    expect(formatUnitExpression(result)).toBe('piano^2/day')
    expect(unitsEqual(result, parseUnitExpression('pianos/day'))).toBe(false)
  })

  it('rejects malformed or non-integral unit expressions', () => {
    expect(() => parseUnitExpression('piano//day')).toThrow('Expected a unit name')
    expect(() => parseUnitExpression('piano^1.5')).toThrow()
  })
})