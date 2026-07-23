import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import NodeInspector from './NodeInspector.vue'
import type { GraphNode } from '../api/types'

const factor: GraphNode = {
  id: 'A',
  revision: 0,
  name: 'flow',
  normalized_name: 'flow',
  title: 'Flow',
  description: '',
  aliases: [],
  metadata: {},
  payload: {
    kind: 'factor',
    properties: { controllable: false, evidence: [] },
  },
}

describe('NodeInspector readiness', () => {
  it('routes a missing quantity to native state setup', async () => {
    const wrapper = mount(NodeInspector, { props: { node: factor, edges: [] } })
    expect(wrapper.text()).toContain('Simulation blocked')
    await wrapper.get('.readiness-actions button').trigger('click')
    expect(wrapper.emitted('quantity')).toHaveLength(1)
  })

  it('offers native state configuration before estimates are authored', async () => {
    const wrapper = mount(NodeInspector, { props: { node: factor, edges: [] } })
    const button = wrapper.findAll('.inspector-actions button').find((item) => item.text().includes('Native state'))!
    await button.trigger('click')
    expect(wrapper.emitted('quantity')).toHaveLength(1)
  })

  it('labels native state estimates with their quantity', () => {
    const quantity = {
      unit: 'days', dimension: { day: 1 }, aggregation: null,
      support: { type: 'bounded' as const, lower: 0, upper: 30 },
    }
    const node: GraphNode = {
      ...factor,
      native_state: {
        quantity,
        current: {
          id: 'A', revision: 0, distribution: { type: 'point', value: 12 }, quantity,
          source: {
            type: 'squiggle',
            definition: { source: 'pointMass(12)', seed: 42, sample_count: 256, target_unit: { day: 1 } },
            },
        },
        forecast: null,
      },
    }

    const wrapper = mount(NodeInspector, { props: { node, edges: [] } })
    expect(wrapper.text()).toContain('State model days')
    expect(wrapper.text()).toContain('Support0 to 30')
  })

  it('orders model work before relationships, details, and deletion', () => {
    const node: GraphNode = {
      ...factor,
      native_state: {
        quantity: { unit: 'days', dimension: { day: 1 }, aggregation: null },
        current: null,
        forecast: null,
      },
    }
    const wrapper = mount(NodeInspector, { props: { node, edges: [] } })
    const text = wrapper.text()
    expect(text.indexOf('State model')).toBeLessThan(text.indexOf('Relationships'))
    expect(text.indexOf('Relationships')).toBeLessThan(text.indexOf('Identity and metadata'))
    expect(text.indexOf('Identity and metadata')).toBeLessThan(text.indexOf('Delete node'))
    expect(wrapper.get('details').attributes('open')).toBeUndefined()
  })
})