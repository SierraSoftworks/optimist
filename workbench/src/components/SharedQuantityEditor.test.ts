import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import type { EstimateAddress, ProjectDependenceModel } from '../api/types'
import type { CatalogueEntry } from '../domain/estimateCatalogue'
import SharedQuantityEditor from './SharedQuantityEditor.vue'

const address: EstimateAddress = {
  project: 'A',
  owner: { kind: 'node', id: 'B' },
  estimate: 'A',
}

const partner: CatalogueEntry = {
  address: { project: 'A', owner: { kind: 'node', id: 'C' }, estimate: 'A' },
  label: 'Recovery time · Current',
  unit: 'minute',
  source: 'normal({ p10: 15, p90: 2880 })',
}

const mismatched: CatalogueEntry = {
  address: { project: 'A', owner: { kind: 'node', id: 'D' }, estimate: 'A' },
  label: 'Change frequency · Current',
  unit: 'change/month',
  source: 'normal({ p10: 1, p90: 10 })',
}

function coupled(): ProjectDependenceModel {
  return {
    revision: 0,
    residual_groups: [{
      members: [address, partner.address],
      correlation: { scale: 'latent', matrix: [[1, 1], [1, 1]] },
    }],
  }
}

function editor(props: Partial<InstanceType<typeof SharedQuantityEditor>['$props']> = {}) {
  return mount(SharedQuantityEditor, {
    props: {
      address,
      unit: 'minute',
      source: 'normal({ p10: 15, p90: 2880 })',
      catalogue: [partner, mismatched],
      dependence: null,
      pending: false,
      ...props,
    },
  })
}

describe('SharedQuantityEditor', () => {
  it('offers only estimates measured in the same unit', () => {
    const options = editor().findAll('option')
    expect(options.map((option) => option.text())).toEqual([
      'Choose an estimate…',
      'Recovery time · Current',
    ])
  })

  it('emits the chosen partner so its source can be adopted', async () => {
    const wrapper = editor()
    await wrapper.get('select').setValue('node/C/A')
    await wrapper.get('button').trigger('click')
    expect(wrapper.emitted('share')?.[0]?.[0]).toEqual(partner)
  })

  it('lists partners and offers to stop sharing once coupled', async () => {
    const wrapper = editor({ dependence: coupled() })
    expect(wrapper.find('select').exists()).toBe(false)
    expect(wrapper.get('.partner-list').text()).toContain('Recovery time · Current')
    await wrapper.get('button').trigger('click')
    expect(wrapper.emitted('unshare')).toHaveLength(1)
  })

  it('warns when a coupled definition has drifted from its partner', () => {
    const matching = editor({ dependence: coupled() })
    expect(matching.find('.form-warning').exists()).toBe(false)

    const drifted = editor({ dependence: coupled(), source: 'normal({ p10: 20, p90: 3000 })' })
    expect(drifted.get('.form-warning').text()).toContain('Recovery time · Current')
  })

  it('renders nothing until the estimate exists and has an address', () => {
    expect(editor({ address: null }).find('.shared-quantity').exists()).toBe(false)
  })
})
