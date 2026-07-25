import { describe, expect, it } from 'vitest'
import type { GraphEdge, GraphNode } from '../api/types'
import { commandPreview, commandSuggestions, parseCommand } from './commandBar'

const nodes = [
  { id: 'A', name: 'automation', title: 'Automation', payload: { kind: 'intervention' } },
  {
    id: 'B', name: 'flow', title: 'Review flow', payload: { kind: 'factor' },
    native_state: { quantity: { unit: 'state', dimension: {}, aggregation: null }, current: null, forecast: null },
  },
] as GraphNode[]

describe('command bar grammar', () => {
  it('creates typed nodes with probabilistic fields awaiting Squiggle setup', () => {
    const result = parseCommand('add factor "Fast feedback" controllable', nodes, [])
    expect(result.command).toMatchObject({
      type: 'create_node',
      input: {
        name: 'fast_feedback',
        payload: { kind: 'factor', properties: { controllable: true } },
      },
    })
    expect(commandPreview(result.command!)).toContainEqual(['Setup', 'State quantity + current estimate required'])
  })

  it('validates relationship endpoints, values, and duplicates', () => {
    expect(parseCommand('connect A changes B 0.4', nodes, []).command).toMatchObject({
      type: 'create_edge',
      input: { source: 'A', destination: 'B', payload: { kind: 'changes' } },
    })
    expect(commandPreview(parseCommand('connect A changes B 0.4', nodes, []).command!))
      .toContainEqual(['Multiplier', '0.4'])
    expect(parseCommand('connect B changes A 0.4', nodes, []).diagnostic.severity).toBe('error')
    expect(parseCommand('connect A changes B nonsense', nodes, []).diagnostic.message).toContain('finite number')
    expect(parseCommand('connect A changes B', nodes, [{ source: 'A', destination: 'B', payload: { kind: 'changes' } } as GraphEdge]).diagnostic.message).toContain('already exists')
  })

  it('connects causal relationships across unrelated endpoint units', () => {
    const metric = {
      id: 'C', revision: 0, name: 'cycle_time', normalized_name: 'cycle_time', title: 'Cycle time',
      description: '', aliases: [], metadata: {},
      payload: { kind: 'metric', properties: { quantity: { unit: 'days', dimension: { day: 1 }, aggregation: null } } },
    } as GraphNode
    const graph = [...nodes, metric]
    expect(parseCommand('connect B contributes C', graph, []).diagnostic.message).toContain('elasticity')
    const result = parseCommand('connect B contributes C -0.5', graph, [])
    expect(result.command).toMatchObject({
      type: 'create_edge',
      input: {
        payload: {
          kind: 'contributes',
          properties: {
            response: {
              id: 'A',
              source: { definition: { source: 'pointMass(-0.5)', target_unit: {} } },
            },
          },
        },
      },
    })
    expect(commandPreview(result.command!)).toContainEqual(['Elasticity', '-0.5'])
  })

  it('provides context-aware suggestions and previews', () => {
    expect(commandSuggestions('add f', nodes)[0]?.label).toBe('factor')
    expect(commandSuggestions('select ', nodes)).toHaveLength(2)
    expect(commandSuggestions('connect A ', nodes).map((item) => item.label)).toEqual([
      'Changes',
      'Requires',
    ])
    expect(commandSuggestions('connect B ', nodes).map((item) => item.label)).toEqual([
      'Requires',
      'Blocks',
    ])
    expect(commandSuggestions('connect A conf', nodes)).toEqual([])
    expect(commandSuggestions('connect A changes ', nodes)[0]?.detail).toBe('factor')
    const result = parseCommand('select B', nodes, [])
    expect(commandPreview(result.command!)).toContainEqual(['Node', 'Review flow · B'])
  })

  it('reports incomplete quoted values and metric units', () => {
    expect(parseCommand('add factor "Fast', nodes, []).diagnostic.message).toContain('Close')
    expect(parseCommand('add metric "Cycle time"', nodes, []).diagnostic.message).toContain('unit')
    expect(parseCommand('add metric "Cycle time" days', nodes, []).command).toMatchObject({
      type: 'create_node',
      input: {
        payload: {
          kind: 'metric',
          properties: { quantity: { unit: 'days', dimension: { day: 1 } } },
        },
      },
    })
  })
})