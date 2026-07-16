import { describe, expect, it } from 'vitest'
import type { EdgeKind, NodeKind } from '../api/types'
import { edgePayload, endpointsAreValid } from './edgeAuthoring'

const valid: Array<[EdgeKind, NodeKind, NodeKind]> = [
  ['contributes', 'factor', 'outcome'],
  ['measures', 'metric', 'factor'],
  ['changes', 'intervention', 'factor'],
  ['requires', 'factor', 'intervention'],
  ['part_of', 'factor', 'factor'],
  ['blocks', 'factor', 'intervention'],
  ['conflicts_with', 'intervention', 'intervention'],
  ['synergizes_with', 'intervention', 'intervention'],
]

describe('edge authoring', () => {
  it.each(valid)('accepts %s from %s to %s', (kind, source, destination) => {
    expect(endpointsAreValid(kind, source, destination)).toBe(true)
  })

  it('rejects representative invalid endpoint combinations', () => {
    expect(endpointsAreValid('measures', 'factor', 'outcome')).toBe(false)
    expect(endpointsAreValid('changes', 'factor', 'factor')).toBe(false)
    expect(endpointsAreValid('part_of', 'outcome', 'factor')).toBe(false)
    expect(endpointsAreValid('conflicts_with', 'intervention', 'factor')).toBe(false)
  })

  it('constructs causal estimates, lag, mechanism, and evidence', () => {
    expect(
      edgePayload({
        kind: 'contributes',
        effect: -0.4,
        lag: 2,
        mechanism: 'Delayed influence',
        evidence: 'ADR-1\nExperiment',
        polarity: 'higher_is_better',
        hard: true,
        threshold: null,
      }),
    ).toEqual({
      kind: 'contributes',
      properties: {
        effect: {
          id: 'A',
          revision: 0,
          distribution: { type: 'point', value: -0.4 },
          provenance: [],
        },
        lag: {
          id: 'B',
          revision: 0,
          distribution: { type: 'point', value: 2 },
          provenance: [],
        },
        mechanism: 'Delayed influence',
        evidence: ['ADR-1', 'Experiment'],
      },
    })
  })

  it('constructs measurement, requirement, blocking, and symmetric payloads', () => {
    const input = {
      effect: -0.8,
      lag: null,
      mechanism: '',
      evidence: '',
      polarity: 'lower_is_better' as const,
      hard: false,
      threshold: 0.75,
    }
    expect(edgePayload({ kind: 'measures', ...input })).toMatchObject({
      kind: 'measures',
      properties: { polarity: 'lower_is_better', observations: [] },
    })
    expect(edgePayload({ kind: 'requires', ...input })).toMatchObject({
      properties: { hard: false, satisfaction_threshold: 0.75 },
    })
    expect(edgePayload({ kind: 'blocks', ...input })).toMatchObject({
      properties: { degree: { distribution: { value: -0.8 } } },
    })
    expect(edgePayload({ kind: 'conflicts_with', ...input })).toEqual({ kind: 'conflicts_with' })
    expect(edgePayload({ kind: 'synergizes_with', ...input })).toEqual({ kind: 'synergizes_with' })
  })
})
