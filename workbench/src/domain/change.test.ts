import { describe, expect, it } from 'vitest'

import { describeChange, directionOf } from './change'

describe('describeChange', () => {
  it('reads a modest change as a share', () => {
    expect(describeChange(100, 120)).toBe('+20%')
    expect(describeChange(100, 80)).toBe('\u221220%')
  })

  /**
   * A share of a number near zero is arithmetically true and useless.
   *
   * A success rate that went from a millionth to nearly one is a hundred million
   * per cent higher. Nobody can hold that figure; everybody can hold "a million
   * times".
   */
  it('switches to a multiple once the two are orders of magnitude apart', () => {
    expect(describeChange(0.000001, 1)).toMatch(/^\u00d7/)
    expect(describeChange(1, 0.000001)).toMatch(/^\u00f7/)
    expect(describeChange(1, 1000)).toBe('\u00d71k')
  })

  /** Nothing has no proportion, so it is named rather than divided by. */
  it('names a change from zero rather than dividing by it', () => {
    expect(describeChange(0, 5)).toBe('from nothing')
    expect(describeChange(0, -5)).toBe('to nothing')
    expect(describeChange(0, 0)).toBeNull()
  })

  it('says nothing about a difference the solver could have invented', () => {
    expect(describeChange(100, 100.5)).toBeNull()
    expect(describeChange(100, 100)).toBeNull()
  })

  it('reports nothing for a figure that is not a number', () => {
    expect(describeChange(Number.NaN, 1)).toBeNull()
    expect(describeChange(1, Number.POSITIVE_INFINITY)).toBeNull()
  })
})

describe('directionOf', () => {
  it('points the way the quantity went', () => {
    expect(directionOf(1, 2)).toBe('up')
    expect(directionOf(2, 1)).toBe('down')
  })

  it('stays quiet where nothing meaningful moved', () => {
    expect(directionOf(100, 100.2)).toBeNull()
    expect(directionOf(5, 5)).toBeNull()
  })
})
