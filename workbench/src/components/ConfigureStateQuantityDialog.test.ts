import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import type { GraphNode } from '../api/types'
import ConfigureStateQuantityDialog from './ConfigureStateQuantityDialog.vue'

const factor: GraphNode = {
  id: 'A', revision: 0, name: 'flow', normalized_name: 'flow', title: 'Flow',
  description: '', aliases: [], metadata: {},
  payload: {
    kind: 'factor',
    properties: { controllable: false, evidence: [] },
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

  it('prefills and edits an existing native quantity type', async () => {
    const existing = {
      ...factor,
      native_state: {
        quantity: {
          unit: 'changes/month',
          dimension: { change: 1, month: -1 },
          aggregation: 'total monthly',
          support: { type: 'non_negative' as const },
          operational_definition: 'Completed changes each month',
          reference_time: null,
          resolution_source: 'Delivery dashboard',
        },
        current: null,
        forecast: null,
      },
    }
    const wrapper = mount(ConfigureStateQuantityDialog, {
      props: { open: true, pending: false, node: existing },
      global: { stubs: { Teleport: true } },
    })

    expect(wrapper.text()).toContain('Edit state type')
    expect(wrapper.get('input').element.value).toBe('changes/month')
    await wrapper.get('select').setValue('real')
    await wrapper.get('form').trigger('submit')

    expect(wrapper.emitted('submit')?.[0]?.[0]).toMatchObject({
      quantity: {
        unit: 'changes/month',
        dimension: { change: 1, month: -1 },
        support: { type: 'real' },
      },
    })
  })
})