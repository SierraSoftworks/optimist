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
})