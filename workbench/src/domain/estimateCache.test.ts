import { describe, expect, it } from 'vitest'
import type { Estimate, GraphNode } from '../api/types'
import { setInterventionEstimate, setStateEstimate } from './estimateCache'

const estimate: Estimate = {
  id: 'A', revision: 0,
  distribution: { type: 'point', value: 12 },
  source: { type: 'distribution' },
  provenance: [],
}

function factor(): GraphNode {
  return {
    id: 'A', revision: 0, name: 'flow', normalized_name: 'flow', title: 'Flow',
    description: '', aliases: [], metadata: {},
    payload: {
      kind: 'factor',
      properties: { current: null, desired: null, controllable: false, evidence: [] },
    },
  }
}

describe('estimate cache updates', () => {
  it('updates legacy and native state without a node refetch', () => {
    const legacy = setStateEstimate(factor(), 'current', estimate)
    expect(legacy.payload.kind === 'factor' && legacy.payload.properties.current).toEqual(estimate)

    const native = setStateEstimate({
      ...factor(),
      native_state: {
        quantity: { unit: 'day', dimension: { day: 1 }, aggregation: null },
        current: null,
        forecast: null,
      },
    }, 'desired', estimate)
    expect(native.native_state?.forecast).toEqual(estimate)
  })

  it('updates intervention slots without a node refetch', () => {
    const intervention: GraphNode = {
      ...factor(),
      payload: {
        kind: 'intervention',
        properties: {
          costs: [], duration: null, probability_of_success: null, acceptance_criteria: [],
        },
      },
    }
    const duration = setInterventionEstimate(intervention, { kind: 'duration' }, estimate)
    expect(duration.payload.kind === 'intervention' && duration.payload.properties.duration).toEqual(estimate)

    const cost = setInterventionEstimate(intervention, { kind: 'cost', value: 'usd' }, estimate)
    expect(cost.payload.kind === 'intervention' && cost.payload.properties.costs).toEqual([
      { dimension: 'usd', value: estimate },
    ])
  })
})
