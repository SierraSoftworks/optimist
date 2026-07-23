import { beforeEach, describe, expect, it } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import type { GraphNode } from '../api/types'
import { useWorkbenchStore } from './workbench'

const node: GraphNode = {
  id: 'A',
  revision: 0,
  name: 'fast_feedback',
  normalized_name: 'fast_feedback',
  title: 'Fast feedback',
  description: '',
  aliases: ['learning loop'],
  metadata: {},
  payload: {
    kind: 'factor',
    properties: { controllable: true, evidence: [] },
  },
}

beforeEach(() => setActivePinia(createPinia()))

describe('workbench state', () => {
  it('filters graph nodes by typed kind and searchable identity', () => {
    const store = useWorkbenchStore()
    expect(store.matches(node)).toBe(true)

    store.search = 'learning'
    expect(store.matches(node)).toBe(true)
    store.search = 'unrelated'
    expect(store.matches(node)).toBe(false)

    store.search = ''
    store.toggleKind('factor')
    expect(store.matches(node)).toBe(false)
  })

  it('clears selection when changing projects', () => {
    const store = useWorkbenchStore()
    store.selectNode('A')
    store.selectProject('B')
    expect(store.selectedProjectId).toBe('B')
    expect(store.selectedNodeId).toBeNull()
  })

  it('filters to nodes which still need simulation setup', () => {
    const store = useWorkbenchStore()
    store.toggleSetupOnly()
    expect(store.matches(node)).toBe(true)

    const ready = structuredClone(node)
    ready.native_state = {
      quantity: { unit: 'state', dimension: {}, aggregation: null },
      current: {
        id: 'A', revision: 0, distribution: { type: 'point', value: 0.5 },
        source: {
          type: 'squiggle',
          definition: { source: 'pointMass(0.5)', seed: 42, sample_count: 256, target_unit: {} },
          assessment: {} as never,
        },
      },
      forecast: null,
    }
    expect(store.matches(ready)).toBe(false)
  })
})
