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
    expect(showScaled(0.8365, scale)).toBe('84%')
    expect(showScaled(1, scale)).toBe('100%')
    expect(showScaled(0, scale)).toBe('0%')
  })

  /** Nines are the whole point of a success rate; rounding them off says nothing. */
  it('keeps the nines a success rate is read for', () => {
    const scale = scaleFor('share')
    expect(showScaled(0.9999982, scale)).toBe('99.9998%')
    expect(showScaled(0.99981, scale)).toBe('99.98%')
    expect(showScaled(0.995123, scale)).toBe('99.5%')
    expect(showScaled(0.9023, scale)).toBe('90%')
  })

  /** A failure share of a millionth is not the same event as one of a hundredth. */
  it('keeps the magnitude of a share that is almost nothing', () => {
    const scale = scaleFor('share')
    expect(showScaled(0.0001, scale)).toBe('0.01%')
    expect(showScaled(0.0000023, scale)).toBe('0.0002%')
  })

  /** A share above all of it is a real reading, not one to hide. */
  it('writes a share beyond the whole as it stands', () => {
    expect(showScaled(1.4, scaleFor('share'))).toBe('140%')
  })

  /** Past a millionth a solved share is reporting the sampler, not the design. */
  it('writes a share a hair from an end as that end', () => {
    const scale = scaleFor('share')
    expect(showScaled(0.99999999997, scale)).toBe('100%')
    expect(showScaled(2.5e-11, scale)).toBe('0%')
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
