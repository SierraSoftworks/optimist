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
  ['changes', 'intervention', 'metric'],
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

  it('constructs unit-aware counterfactual responses for metric endpoints', () => {
    const source = {
      id: 'A', revision: 0, name: 'flow', normalized_name: 'flow', title: 'Flow',
      description: '', aliases: [], metadata: {},
      native_state: { quantity: { unit: 'state', dimension: {}, aggregation: null }, current: null, forecast: null },
      payload: { kind: 'factor', properties: { controllable: false, evidence: [] } },
    } as GraphNode
    const destination = {
      id: 'B', revision: 0, name: 'lead_time', normalized_name: 'lead_time', title: 'Lead time',
      description: '', aliases: [], metadata: {},
      payload: { kind: 'metric', properties: { quantity: { unit: 'days', dimension: { day: 1 }, aggregation: null } } },
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

  it('constructs a unit-aware intervention shift for a metric', () => {
    const source = {
      id: 'A', revision: 0, name: 'automation', normalized_name: 'automation', title: 'Automation',
      description: '', aliases: [], metadata: {},
      payload: { kind: 'intervention', properties: { costs: [], duration: null, probability_of_success: null, acceptance_criteria: [] } },
    } as GraphNode
    const destination = {
      id: 'B', revision: 0, name: 'lead_time', normalized_name: 'lead_time', title: 'Lead time',
      description: '', aliases: [], metadata: {},
      payload: { kind: 'metric', properties: { quantity: { unit: 'days', dimension: { day: 1 }, aggregation: null } } },
    } as GraphNode
    const destinationEstimate = {
      id: 'A', revision: 0, distribution: { type: 'point' as const, value: -2 },
      source: {
        type: 'squiggle' as const,
        definition: { source: 'pointMass(-2)', seed: 42, sample_count: 256, target_unit: { day: 1 } },
      },
      provenance: [],
    }
    expect(edgePayload({
      kind: 'changes', effect: 0, lag: null, mechanism: 'Automation reduces delay', evidence: '',
      polarity: 'higher_is_better', hard: true, threshold: null,
      source, destination, sourceChange: 1, destinationEstimate,
    })).toMatchObject({
      kind: 'changes',
      properties: {
        response: {
          source_change: 1,
          source_unit: {},
          destination_change: destinationEstimate,
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
