import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import type { GraphEdge, GraphNode } from '../api/types'
import NodeRelationDialog from './NodeRelationDialog.vue'

function metric(id: string, name: string, unit: Record<string, number>): GraphNode {
  return {
    id, revision: 0, name, normalized_name: name, title: name,
    description: '', aliases: [], metadata: {},
    payload: {
      kind: 'metric',
      properties: { quantity: { unit: name, dimension: unit, aggregation: null } },
    },
  } as unknown as GraphNode
}

const outcome = {
  id: 'C', revision: 0, name: 'impact', normalized_name: 'impact', title: 'Customer impact',
  description: '', aliases: [], metadata: {},
  native_state: {
    quantity: { unit: 'minutes', dimension: { minute: 1 }, aggregation: null },
    current: null,
    forecast: null,
  },
  payload: { kind: 'outcome', properties: { direction: 'minimize', evidence: [] } },
} as unknown as GraphNode

const intervention = {
  id: 'D', revision: 0, name: 'code_yellow', normalized_name: 'code_yellow', title: 'Code yellow',
  description: '', aliases: [], metadata: {},
  payload: { kind: 'intervention', properties: { costs: [], duration: null, probability_of_success: null, acceptance_criteria: [] } },
} as unknown as GraphNode

const nodes = [
  metric('A', 'outage_frequency', { outage: 1 }),
  metric('B', 'impact_duration', { minute: 1, outage: -1 }),
  intervention,
  outcome,
]

const edges = [
  { source: 'A', destination: 'C', payload: { kind: 'contributes' } },
  { source: 'B', destination: 'C', payload: { kind: 'contributes' } },
  { source: 'D', destination: 'C', payload: { kind: 'changes' } },
  { source: 'A', destination: 'B', payload: { kind: 'contributes' } },
] as unknown as GraphEdge[]

function dialog(node: GraphNode = outcome) {
  return mount(NodeRelationDialog, {
    props: { open: true, pending: false, node, nodes, edges },
    global: { stubs: { Teleport: true } },
  })
}

describe('NodeRelationDialog', () => {
  it('offers exactly the names the graph binds for this node', () => {
    const listed = dialog().findAll('.bindings li').map((item) => item.find('code').text())
    expect(listed).toEqual(['baseline', 'outage_frequency', 'impact_duration', 'code_yellow'])
  })

  it('shows each parent unit and the unit the equation must produce', () => {
    const wrapper = dialog()
    expect(wrapper.get('.result-unit').text()).toContain('minute')
    const units = wrapper.findAll('.bindings li').map((item) => item.find('.binding-unit').text())
    expect(units).toEqual(['minute', 'outage', 'minute/outage', '1'])
  })

  it('submits the trimmed calculation', async () => {
    const wrapper = dialog()
    await wrapper.get('textarea').setValue('  outage_frequency * impact_duration  ')
    await wrapper.get('form').trigger('submit')
    expect(wrapper.emitted('submit')?.[0]?.[0]).toEqual({
      relation: { source: 'outage_frequency * impact_duration', parameters: undefined },
    })
  })

  it('refuses to submit an empty calculation', async () => {
    const wrapper = dialog()
    await wrapper.get('form').trigger('submit')
    expect(wrapper.emitted('submit')).toBeUndefined()
  })

  it('seeds an existing equation and can remove it', async () => {
    const existing = JSON.parse(JSON.stringify(outcome)) as GraphNode
    existing.native_state!.relation = { source: 'outage_frequency * impact_duration' }
    const wrapper = dialog(existing)
    expect((wrapper.get('textarea').element as HTMLTextAreaElement).value)
      .toBe('outage_frequency * impact_duration')
    await wrapper.get('button.secondary-button').trigger('click')
    expect(wrapper.emitted('submit')?.[0]?.[0]).toEqual({ relation: null })
  })

  it('requires a canonical quantity before an equation can be written', () => {
    const unmeasured = JSON.parse(JSON.stringify(outcome)) as GraphNode
    delete unmeasured.native_state
    const wrapper = dialog(unmeasured)
    expect(wrapper.find('textarea').exists()).toBe(false)
    expect(wrapper.get('.form-error').text()).toContain('canonical quantity')
  })
})
