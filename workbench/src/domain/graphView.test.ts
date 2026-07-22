import { describe, expect, it } from 'vitest'
import type { GraphNode, NodeKind } from '../api/types'
import { clusteredPositions, defaultGraphLayout, graphDetailForZoom } from './graphView'

function node(id: string, kind: NodeKind): GraphNode {
  const properties = kind === 'metric'
    ? { unit: 'count', aggregation: null }
    : kind === 'intervention'
      ? { costs: [], duration: null, probability_of_success: null, acceptance_criteria: [] }
      : kind === 'outcome'
        ? { direction: 'maximize' as const, current: null, desired: null, evidence: [] }
        : { current: null, desired: null, controllable: false, evidence: [] }
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

  it('defaults dense graphs to clusters', () => {
    expect(defaultGraphLayout(59)).toBe('hierarchy')
    expect(defaultGraphLayout(60)).toBe('clusters')
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