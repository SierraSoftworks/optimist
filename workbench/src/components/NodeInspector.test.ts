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
    properties: { current: null, desired: null, controllable: false, evidence: [] },
  },
}

describe('NodeInspector readiness', () => {
  it('routes a missing current baseline to the estimate editor', async () => {
    const wrapper = mount(NodeInspector, { props: { node: factor, edges: [] } })
    expect(wrapper.text()).toContain('Simulation blocked')
    await wrapper.get('.readiness-actions button').trigger('click')
    expect(wrapper.emitted('estimate')).toHaveLength(1)
  })

  it('offers native state configuration before legacy state is authored', async () => {
    const wrapper = mount(NodeInspector, { props: { node: factor, edges: [] } })
    await wrapper.get('button:nth-child(3)').trigger('click')
    expect(wrapper.emitted('quantity')).toHaveLength(1)
  })

  it('labels legacy normalized estimates as standardized quantities', () => {
    const node: GraphNode = {
      ...factor,
      payload: {
        kind: 'factor',
        properties: {
          current: {
            id: 'A',
            revision: 0,
            distribution: { type: 'beta', alpha: 2, beta: 3 },
            quantity: {
              unit: 'standardized_state',
              dimension: {},
              aggregation: null,
              support: { type: 'bounded', lower: 0, upper: 1 },
              operational_definition: 'Legacy standardized factor or outcome state where 0 and 1 are model-specific anchors.',
            },
          },
          desired: null,
          controllable: false,
          evidence: [],
        },
      },
    }

    const wrapper = mount(NodeInspector, { props: { node, edges: [] } })
    expect(wrapper.text()).toContain('standardized_state · [0, 1]')
  })
})