import { describe, expect, it } from 'vitest'
import type { GraphNode } from '../api/types'
import { readinessLabel, simulationReadiness } from './simulationReadiness'

function node(payload: GraphNode['payload']): GraphNode {
  return {
    id: 'A',
    revision: 0,
    name: 'node',
    normalized_name: 'node',
    title: 'Node',
    description: '',
    aliases: [],
    metadata: {},
    payload,
  }
}

describe('simulationReadiness', () => {
  it('blocks outcomes and factors without a current baseline', () => {
    const readiness = simulationReadiness(node({
      kind: 'factor',
      properties: { controllable: false, evidence: [] },
    }))
    expect(readiness.level).toBe('required')
    expect(readinessLabel(readiness)).toContain('State quantity')
  })

  it('treats intervention planning inputs as recommendations', () => {
    const readiness = simulationReadiness(node({
      kind: 'intervention',
      properties: {
        costs: [],
        duration: null,
        probability_of_success: null,
        acceptance_criteria: [],
      },
    }))
    expect(readiness.level).toBe('recommended')
    expect(readiness.issues.map((issue) => issue.key)).toEqual([
      'success_probability',
      'duration',
    ])
  })

  it('marks configured and metric nodes ready', () => {
    const estimate = {
      id: 'A',
      revision: 0,
      distribution: { type: 'beta' as const, alpha: 2, beta: 2 },
      source: {
        type: 'squiggle' as const,
        definition: { source: 'beta(2, 2)', seed: 42, sample_count: 256, target_unit: {} },
        assessment: {} as never,
      },
    }
    expect(simulationReadiness({
      ...node({
      kind: 'outcome',
      properties: { direction: 'maximize', evidence: [] },
      }),
      native_state: {
        quantity: { unit: 'state', dimension: {}, aggregation: null },
        current: estimate,
        forecast: null,
      },
    }).level).toBe('ready')
    expect(simulationReadiness(node({
      kind: 'metric',
      properties: { quantity: { unit: 'minutes', dimension: { minutes: 1 }, aggregation: null } },
    })).level).toBe('ready')
  })
})