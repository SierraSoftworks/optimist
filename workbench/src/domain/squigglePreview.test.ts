import { describe, expect, it } from 'vitest'
import type { FermiComponentDraft } from './fermiBuilder'
import { evaluateSquigglePreview, SquigglePreviewError } from './squigglePreview'

describe('Squiggle preview', () => {
  it('evaluates Optimist PERT variables as a deterministic distribution', async () => {
    const components = [variable('work', 0, 5, 10)]
    const first = await evaluateSquigglePreview('work * 2', components, 'non_negative', {})
    const second = await evaluateSquigglePreview('work * 2', components, 'non_negative', {})

    expect({ ...first, executionMilliseconds: 0 }).toEqual({ ...second, executionMilliseconds: 0 })
    expect(first.mean).toBeCloseTo(10, 0)
    expect(first.p05).toBeLessThan(first.p50)
    expect(first.p50).toBeLessThan(first.p95)
    expect(first.standardDeviation).toBeGreaterThan(0)
  })

  it('applies Optimist support bounds before summarizing the preview', async () => {
    const preview = await evaluateSquigglePreview('shift', [variable('shift', -1, 0.5, 2)], 'probability', {})

    expect(preview.p05).toBeGreaterThanOrEqual(0)
    expect(preview.p95).toBeLessThanOrEqual(1)
    expect(preview.supportViolationProbability).toBeGreaterThan(0.25)
  })

  it('reports unsupported negative mass without clamping a non-negative estimate', async () => {
    const preview = await evaluateSquigglePreview('shift', [variable('shift', -2, 1, 3)], 'non_negative', {})

    expect(preview.p05).toBeLessThan(0)
    expect(preview.supportViolationProbability).toBeGreaterThan(0)
  })

  it('reports syntax failures relative to the authored equation', async () => {
    await expect(evaluateSquigglePreview('work +', [variable('work', 0, 5, 10)], 'real', {}))
      .rejects.toMatchObject({ line: 1 } satisfies Partial<SquigglePreviewError>)
  })

  it('uses Squiggle annotations to enforce composite variable and result units', async () => {
    const components = [
      variable('rate', 2, 4, 6, 'items/day'),
      variable('time', 1, 2, 3, 'days'),
    ]
    const preview = await evaluateSquigglePreview('rate * time', components, 'real', { item: 1 })
    expect(preview.mean).toBeGreaterThan(0)

    await expect(evaluateSquigglePreview('rate * time', components, 'real', { day: 1 }))
      .rejects.toThrow('Conflicting unit types')
  })
})

function variable(
  name: string,
  low: number,
  likely: number,
  high: number,
  unit = '',
): FermiComponentDraft {
  return { name, low, likely, high, unit, mode: 'pert' }
}