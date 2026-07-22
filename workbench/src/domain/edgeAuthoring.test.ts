import { describe, expect, it } from 'vitest'
import type { EdgeKind, GraphNode, NodeKind } from '../api/types'
import { edgePayload, endpointsAreValid } from './edgeAuthoring'

const valid: Array<[EdgeKind, NodeKind, NodeKind]> = [
  ['contributes', 'factor', 'outcome'],
  ['contributes', 'factor', 'metric'],
  ['contributes', 'metric', 'metric'],
  ['contributes', 'metric', 'outcome'],
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

  it('constructs unit-aware counterfactual responses for metric endpoints', () => {
    const source = {
      id: 'A', revision: 0, name: 'flow', normalized_name: 'flow', title: 'Flow',
      description: '', aliases: [], metadata: {},
      payload: { kind: 'factor', properties: { current: null, desired: null, controllable: false, evidence: [] } },
    } as GraphNode
    const destination = {
      id: 'B', revision: 0, name: 'lead_time', normalized_name: 'lead_time', title: 'Lead time',
      description: '', aliases: [], metadata: {},
      payload: { kind: 'metric', properties: { unit: 'days', dimension: { day: 1 }, aggregation: null } },
    } as GraphNode
    expect(edgePayload({
      kind: 'contributes', effect: 0, lag: null, mechanism: 'Flow reduces delay', evidence: '',
      polarity: 'higher_is_better', hard: true, threshold: null,
      source, destination, sourceChange: 0.1, destinationChange: -2,
    })).toMatchObject({
      kind: 'contributes',
      properties: {
        response: {
          source_change: 0.1,
          source_unit: {},
          destination_change: { distribution: { type: 'point', value: -2 } },
          destination_unit: { day: 1 },
        },
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
