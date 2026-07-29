import { describe, expect, it } from 'vitest'
import { formatHumanNumber, formatSiNumber, parseHumanNumber } from './humanNumber'

describe('human engineering numbers', () => {
  it('parses compact suffixes, commas, and scientific notation', () => {
    expect(parseHumanNumber('1.5M')).toBe(1_500_000)
    expect(parseHumanNumber('2,400')).toBe(2_400)
    expect(parseHumanNumber('2.4e3')).toBe(2_400)
    expect(formatHumanNumber(1_500_000)).toBe('1.5M')
  })

  it('rejects ambiguous or non-finite values', () => {
    expect(() => parseHumanNumber('about 20')).toThrow('Use a number')
    expect(() => parseHumanNumber('Infinity')).toThrow()
  })
})

describe('formatSiNumber', () => {
  it.each([
    [1_370_000, '1.37M'],
    [1370, '1.37k'],
    [1, '1'],
    [0.000137, '137µ'],
    [0, '0'],
    [Number.NaN, '—'],
  ])('writes %d with an SI prefix', (value, expected) => {
    expect(formatSiNumber(value)).toBe(expected)
  })

  /** Dividing before rounding carries the mantissa into the next prefix. */
  it.each([
    [0.9999, '1'],
    [999.9, '1k'],
    [-0.9999, '-1'],
  ])('promotes %d rather than writing a mantissa of a thousand', (value, expected) => {
    expect(formatSiNumber(value)).toBe(expected)
  })
})