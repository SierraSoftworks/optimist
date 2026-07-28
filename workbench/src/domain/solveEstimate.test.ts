import { beforeEach, describe, expect, it } from 'vitest'

import { expected, forget, progress, remaining, remember } from './solveEstimate'

beforeEach(forget)

describe('remember', () => {
  it('reports the first measurement as the expectation', () => {
    remember('shape', 400)
    expect(expected('shape')).toBe(400)
  })

  it('moves toward later measurements without jumping to them', () => {
    remember('shape', 400)
    remember('shape', 900)
    const settled = expected('shape')
    expect(settled).toBeGreaterThan(400)
    expect(settled).toBeLessThan(900)
  })

  /**
   * A suspended tab reports the wall clock, not the work.
   *
   * Closing a laptop lid midway through a request would otherwise teach the
   * estimator that solving this design takes an hour, and every solve afterwards
   * would promise one.
   */
  it('ignores a measurement no solve could have produced', () => {
    remember('shape', 400)
    remember('shape', 60 * 60 * 1000)
    expect(expected('shape')).toBe(400)
  })

  it('knows nothing about a shape it has not seen', () => {
    expect(expected('unseen')).toBeNull()
  })
})

describe('progress', () => {
  it('is most of the way along when the expected time has passed', () => {
    expect(progress(0, 1000)).toBe(0)
    expect(progress(500, 1000)).toBeCloseTo(0.45)
    expect(progress(1000, 1000)).toBeCloseTo(0.9)
  })

  /**
   * A bar that fills before the answer arrives is a bar nobody believes again.
   */
  it('approaches the end without reaching it, however long the solve runs', () => {
    expect(progress(5_000, 1000)).toBeGreaterThan(0.9)
    expect(progress(100_000, 1000)).toBeLessThan(1)
    expect(progress(100_000, 1000)).toBeGreaterThan(progress(5_000, 1000))
  })
})

describe('remaining', () => {
  it('counts down in seconds', () => {
    expect(remaining(1000, 4000)).toBe(3)
  })

  it('reports nothing once the solve has outrun its estimate', () => {
    expect(remaining(5000, 4000)).toBeNull()
  })
})
