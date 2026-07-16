import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import type { GraphEdge } from '../api/types'
import EditEdgeDialog from './EditEdgeDialog.vue'

const edge: GraphEdge = {
  source: 'A', source_kind: 'metric', destination: 'B', destination_kind: 'factor',
  revision: 0, description: '', metadata: {},
  payload: {
    kind: 'measures',
    properties: { polarity: 'lower_is_better', observations: [] },
  },
}

describe('EditEdgeDialog', () => {
  it('authors metric anchors which explain normalized state', async () => {
    const wrapper = mount(EditEdgeDialog, {
      props: { open: true, pending: false, edge },
      global: { stubs: { Teleport: true } },
    })
    await wrapper.get('.calibration-editor input[type="checkbox"]').setValue(true)
    const inputs = wrapper.findAll('.calibration-editor input[type="number"]')
    await inputs[0]!.setValue('20')
    await inputs[1]!.setValue('5')
    expect(wrapper.text()).toContain('20 metric units → state 0')
    expect(wrapper.text()).toContain('5 metric units → state 1')
    await wrapper.get('.calibration-editor > .secondary-button').trigger('click')
    expect(wrapper.emitted('calibration')![0]).toEqual([{
      calibration: { type: 'linear', state_zero: 20, state_one: 5 },
    }])
  })
})