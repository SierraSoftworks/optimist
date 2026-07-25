import { describe, expect, it } from 'vitest'
import type { GraphNode, NodeKind } from '../api/types'
import { clusteredPositions, defaultGraphLayout, graphDetailForZoom } from './graphView'

function node(id: string, kind: NodeKind): GraphNode {
  const properties = kind === 'metric'
    ? { quantity: { unit: 'count', dimension: { count: 1 }, aggregation: null } }
    : kind === 'intervention'
      ? { costs: [], duration: null, probability_of_success: null, acceptance_criteria: [] }
      : kind === 'outcome'
        ? { direction: 'maximize' as const, evidence: [] }
        : { controllable: false, evidence: [] }
  return {
    id,
    revision: 0,
    name: id.toLocaleLowerCase(),
    normalized_name: id.toLocaleLowerCase(),
    title: id,
    description: '',
    aliases: [],
    metadata: {},
    payload: { kind, properties } as GraphNode['payload'],
  }
}

describe('graph view policy', () => {
  it('uses stable semantic zoom thresholds', () => {
    expect(graphDetailForZoom(0.4)).toBe('overview')
    expect(graphDetailForZoom(0.62)).toBe('context')
    expect(graphDetailForZoom(0.94)).toBe('detail')
  })

  /**
   * The force layout is bounded at every size, so density no longer decides the
   * opening view; clustering by kind is a deliberate choice rather than a
   * fallback for graphs the old layout could not cope with.
   */
  it('opens every model in the causal layout', () => {
    expect(defaultGraphLayout(3)).toBe('hierarchy')
    expect(defaultGraphLayout(60)).toBe('hierarchy')
    expect(defaultGraphLayout(500)).toBe('hierarchy')
  })

  it('places node kinds into ordered non-overlapping bands', () => {
    const positions = clusteredPositions([
      node('I', 'intervention'),
      node('F1', 'factor'),
      node('F2', 'factor'),
      node('M', 'metric'),
      node('O', 'outcome'),
    ])
    expect(positions.get('I')!.y).toBeLessThan(positions.get('F1')!.y)
    expect(positions.get('F2')!.x).not.toBe(positions.get('F1')!.x)
    expect(positions.get('F1')!.y).toBeLessThan(positions.get('M')!.y)
    expect(positions.get('M')!.y).toBeLessThan(positions.get('O')!.y)
  })
})