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

  it('reopens a time-boxed effect whose ending was stored as the default', async () => {
    const periods = (value: number) => ({
      id: 'A',
      revision: 0,
      source: { type: 'squiggle' as const, definition: { source: `pointMass(${value})`, seed: 42, sample_count: 256, target_unit: { duration: 1 } } },
    })
    const timeBoxed: GraphEdge = {
      ...edge,
      source_kind: 'intervention',
      payload: {
        kind: 'changes',
        properties: {
          response: periods(0.25),
          lag: null,
          mechanism: 'A code yellow suspends discretionary change.',
          evidence: [],
          // An abrupt ending is the server's default, so it omits `release`.
          transience: {
            profile: {
              hold: periods(3),
              aftereffect: { hold: periods(1), release: { type: 'immediate' } },
            },
            rebound: periods(1.25),
          },
        },
      },
    }
    const wrapper = mount(EditEdgeDialog, {
      props: { open: true, pending: false, edge: timeBoxed },
      global: { stubs: { Teleport: true } },
    })
    const timeBox = wrapper.get('.effect-profile input[type="checkbox"]')
    expect((timeBox.element as HTMLInputElement).checked).toBe(true)
    const holdField = wrapper.findAll('.effect-profile input[type="number"]')[1]!
    expect((holdField.element as HTMLInputElement).value).toBe('3')
    expect((wrapper.get('.effect-profile select').element as HTMLSelectElement).value).toBe('immediate')
    expect((wrapper.find('textarea').element as HTMLTextAreaElement).value).toContain('code yellow')
  })
})