import { describe, expect, it } from 'vitest'

import { scaleFor, showScaled, showWithUnit } from './units'

describe('scaleFor', () => {
  it('reads a dimensionless quantity inside zero and one as a proportion', () => {
    expect(scaleFor('1', [0.1, 0.87, 0.99])).toEqual({ factor: 100, suffix: '%' })
    expect(scaleFor('', [0, 1])).toEqual({ factor: 100, suffix: '%' })
  })

  /**
   * Not every dimensionless quantity is a proportion. A retry multiplier and a
   * call depth are both declared `1`, and showing 3 attempts as 300% would be
   * worse than showing nothing.
   */
  it('leaves a dimensionless quantity above one alone', () => {
    expect(scaleFor('1', [1, 2.4, 8])).toEqual({ factor: 1, suffix: '' })
  })

  it('leaves a negative dimensionless quantity alone', () => {
    expect(scaleFor('1', [-0.2, 0.5])).toEqual({ factor: 1, suffix: '' })
  })

  it('keeps a real unit as its own label', () => {
    expect(scaleFor('op/s', [1, 900])).toEqual({ factor: 1, suffix: 'op/s' })
    expect(scaleFor('s', [0.001, 2])).toEqual({ factor: 1, suffix: 's' })
  })

  it('survives having nothing to look at', () => {
    expect(scaleFor('1', [])).toEqual({ factor: 1, suffix: '' })
    expect(scaleFor('1', [Number.NaN])).toEqual({ factor: 1, suffix: '' })
  })
})

describe('showScaled', () => {
  it('writes a proportion as a percentage', () => {
    const scale = scaleFor('1', [0, 1])
    expect(showScaled(0.8365, scale)).toBe('83.7%')
    expect(showScaled(1, scale)).toBe('100%')
    expect(showScaled(0, scale)).toBe('0%')
  })

  it('leaves other quantities to the magnitude prefix', () => {
    const scale = scaleFor('s', [0.0001, 1])
    expect(showScaled(0.000123, scale)).toBe('123µ')
  })

  it('says nothing rather than NaN', () => {
    expect(showScaled(Number.NaN, { factor: 1, suffix: '' })).toBe('—')
  })
})

describe('showWithUnit', () => {
  it('writes the unit out where there is room', () => {
    expect(showWithUnit(900, scaleFor('op/s', [900]))).toBe('900 op/s')
  })

  /** A percent sign is already attached, so a second one would read `83% %`. */
  it('does not repeat a percent sign', () => {
    expect(showWithUnit(0.83, scaleFor('1', [0, 1]))).toBe('83%')
  })
})
