import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import type { GraphNode } from '../api/types'
import ConfigureStateQuantityDialog from './ConfigureStateQuantityDialog.vue'

const factor: GraphNode = {
  id: 'A', revision: 0, name: 'flow', normalized_name: 'flow', title: 'Flow',
  description: '', aliases: [], metadata: {},
  payload: {
    kind: 'factor',
    properties: { current: null, desired: null, controllable: false, evidence: [] },
  },
}

describe('ConfigureStateQuantityDialog', () => {
  it('authors a canonical bounded native quantity', async () => {
    const wrapper = mount(ConfigureStateQuantityDialog, {
      props: { open: true, pending: false, node: factor },
      global: { stubs: { Teleport: true } },
    })
    const inputs = wrapper.findAll('input')
    await inputs[0]!.setValue('day')
    await wrapper.get('select').setValue('bounded')
    const bounded = wrapper.findAll('input')
    await bounded[2]!.setValue('0')
    await bounded[3]!.setValue('30')
    await wrapper.get('textarea').setValue('Elapsed lead time')
    await wrapper.get('form').trigger('submit')

    expect(wrapper.emitted('submit')?.[0]?.[0]).toMatchObject({
      quantity: {
        unit: 'day',
        dimension: { day: 1 },
        support: { type: 'bounded', lower: 0, upper: 30 },
        operational_definition: 'Elapsed lead time',
      },
    })
  })

  it('requires explicit zero and one anchors for populated legacy state', async () => {
    const populated: GraphNode = {
      ...factor,
      payload: {
        kind: 'factor',
        properties: {
          current: {
            id: 'A', revision: 0,
            distribution: { type: 'point', value: 0.5 },
          },
          desired: null,
          controllable: false,
          evidence: [],
        },
      },
    }
    const wrapper = mount(ConfigureStateQuantityDialog, {
      props: { open: true, pending: false, node: populated },
      global: { stubs: { Teleport: true } },
    })
    const fields = wrapper.findAll('input')
    await fields[0]!.setValue('day')
    await wrapper.get('input[type="number"]').setValue('10')
    await wrapper.findAll('input[type="number"]')[1]!.setValue('30')
    await wrapper.get('form').trigger('submit')

    expect(wrapper.emitted('submit')?.[0]?.[0]).toMatchObject({
      legacy_mapping: { state_zero: 10, state_one: 30 },
    })
  })
})