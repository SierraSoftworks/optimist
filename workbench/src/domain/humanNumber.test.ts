import { describe, expect, it } from 'vitest'
import { formatHumanNumber, parseHumanNumber } from './humanNumber'

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