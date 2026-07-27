import { describe, expect, it } from 'vitest'
import { outcomeScale, positionOn } from './outcomeScale'

describe('outcomeScale', () => {
  it('places values by logarithm when the outcome stays positive', () => {
    const scale = outcomeScale([0.02, 1, 150])
    expect(scale.logarithmic).toBe(true)
    expect(scale.lower).toBeLessThan(0.02)
    expect(scale.upper).toBeGreaterThan(150)
  })

  /**
   * Four decades is the range that made the linear chart unreadable: the resting
   * behaviour has to stay visible next to the collapse.
   */
  it('gives the resting decades room next to a saturating one', () => {
    const scale = outcomeScale([0.02, 0.04, 150])
    const resting = positionOn(scale, 0.02)
    const doubled = positionOn(scale, 0.04)
    expect(doubled - resting).toBeGreaterThan(0.05)
  })

  it('falls back to a linear axis when a plotted line goes below zero', () => {
    expect(outcomeScale([-5, 0, 5]).logarithmic).toBe(false)
  })

  /**
   * An uncertainty band on a non-negative quantity reaches zero constantly, and
   * letting that decide would put every such chart on a linear axis over a range
   * that needs a logarithm.
   */
  it('keeps a logarithm when only the uncertainty band reaches zero', () => {
    const scale = outcomeScale([0.02, 1, 150], [0, 0, 60])
    expect(scale.logarithmic).toBe(true)
    expect(scale.lower).toBeGreaterThan(0)
  })

  /**
   * A propagated quantity can pass through exact zero for a period; that must
   * not stretch the axis until everything real collapses onto one line.
   */
  it('bounds how far a zero period may stretch the axis', () => {
    const scale = outcomeScale([0, 1.77e-13, 0.02, 195])
    expect(scale.logarithmic).toBe(true)
    expect(Math.log10(scale.upper) - Math.log10(scale.lower)).toBeLessThan(8)
    expect(positionOn(scale, 0)).toBe(0)
    expect(positionOn(scale, 0.02)).toBeGreaterThan(0.2)
  })

  it('orders positions so that up is always more of the quantity', () => {
    for (const values of [[0.5, 2, 8], [-4, 0, 4]]) {
      const scale = outcomeScale(values)
      const positions = values.map((value) => positionOn(scale, value))
      expect(positions).toEqual([...positions].sort((a, b) => a - b))
    }
  })

  it('keeps a flat series on the axis rather than dividing by an empty span', () => {
    const scale = outcomeScale([7, 7, 7])
    const position = positionOn(scale, 7)
    expect(position).toBeGreaterThan(0)
    expect(position).toBeLessThan(1)
  })

  it('survives a series with nothing to plot', () => {
    const scale = outcomeScale([null, undefined, Number.NaN])
    expect(Number.isFinite(positionOn(scale, 1))).toBe(true)
  })
})
