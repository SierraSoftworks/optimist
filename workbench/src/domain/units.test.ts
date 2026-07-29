import { describe, expect, it } from 'vitest'

import { scaleFor, showScaled, showWithUnit } from './units'

describe('scaleFor', () => {
  it.each(['share', 'ratio', 'fraction', 'proportion', 'probability', '%'])(
    'reads %s as a proportion',
    (unit) => {
      expect(scaleFor(unit)).toEqual({ factor: 100, suffix: '%' })
    },
  )

  /**
   * Not every dimensionless quantity is a proportion. A retry multiplier and a
   * call depth are both declared `1`, and showing 3 attempts as 300% would be
   * worse than showing nothing.
   */
  it('writes a dimensionless quantity as a bare number', () => {
    expect(scaleFor('1')).toEqual({ factor: 1, suffix: '' })
    expect(scaleFor('')).toEqual({ factor: 1, suffix: '' })
  })

  it('keeps a real unit as its own label', () => {
    expect(scaleFor('op/s')).toEqual({ factor: 1, suffix: 'op/s' })
    expect(scaleFor('s')).toEqual({ factor: 1, suffix: 's' })
  })
})

describe('showScaled', () => {
  it('writes a proportion as a percentage', () => {
    const scale = scaleFor('share')
    expect(showScaled(0.8365, scale)).toBe('83.7%')
    expect(showScaled(1, scale)).toBe('100%')
    expect(showScaled(0, scale)).toBe('0%')
  })

  /** A share above all of it is a real reading, not one to hide. */
  it('writes a share beyond the whole as it stands', () => {
    expect(showScaled(1.4, scaleFor('share'))).toBe('140%')
  })

  it('leaves other quantities to the magnitude prefix', () => {
    const scale = scaleFor('s')
    expect(showScaled(0.000123, scale)).toBe('123µ')
  })

  it('says nothing rather than NaN', () => {
    expect(showScaled(Number.NaN, { factor: 1, suffix: '' })).toBe('—')
  })
})

describe('showWithUnit', () => {
  it('writes the unit out where there is room', () => {
    expect(showWithUnit(900, scaleFor('op/s'))).toBe('900 op/s')
  })

  /** A percent sign is already attached, so a second one would read `83% %`. */
  it('does not repeat a percent sign', () => {
    expect(showWithUnit(0.83, scaleFor('share'))).toBe('83%')
  })

  it('writes a dimensionless quantity with nothing after it', () => {
    expect(showWithUnit(3, scaleFor('1'))).toBe('3')
  })
})
