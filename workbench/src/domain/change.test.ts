import { describe, expect, it } from 'vitest'

import { describeChange, directionOf, emphasisOf } from './change'

describe('describeChange', () => {
  it('reads a change as a signed share of what it started at', () => {
    expect(describeChange(100, 120)).toBe('+20%')
    expect(describeChange(100, 80)).toBe('\u221220%')
  })

  /**
   * One form, so that two figures side by side can be compared by eye.
   *
   * A mix of shares and multiples makes a reader work out which arithmetic each
   * figure used before they can tell which of the two moved further.
   */
  it('keeps the same form however far the quantity moved', () => {
    expect(describeChange(1, 1000)).toBe('+100k%')
    expect(describeChange(0.000001, 1)).toMatch(/^\+\d/)
    expect(describeChange(1, 0.000001)).toBe('\u2212100%')
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

describe('emphasisOf', () => {
  it('marks out a movement worth opening', () => {
    expect(emphasisOf(100, 250)).toBe('notable')
    expect(emphasisOf(100, 40)).toBe('notable')
    expect(emphasisOf(0, 5)).toBe('notable')
  })

  /** A doubling and the halving that undoes it are the same size of event. */
  it('reads a rise and the fall that undoes it alike', () => {
    expect(emphasisOf(100, 200)).toBe(emphasisOf(200, 100))
    expect(emphasisOf(100, 150)).toBe(emphasisOf(150, 100))
  })

  it('keeps a small movement quiet', () => {
    expect(emphasisOf(100, 130)).toBe('slight')
    expect(emphasisOf(100, 80)).toBe('slight')
  })

  it('grades nothing the solver could have invented', () => {
    expect(emphasisOf(100, 100.2)).toBeNull()
    expect(emphasisOf(5, 5)).toBeNull()
    expect(emphasisOf(Number.NaN, 1)).toBeNull()
  })
})
