import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import type { GraphNode } from '../api/types'
import CreateEdgeDialog from './CreateEdgeDialog.vue'

const nodes = [
  { id: 'A', title: 'Source factor', payload: { kind: 'factor' } },
  { id: 'B', title: 'Destination factor', payload: { kind: 'factor' } },
] as GraphNode[]

describe('CreateEdgeDialog', () => {
  it('prefills a relationship kind and source when opened from a node action', async () => {
    const wrapper = mount(CreateEdgeDialog, {
      props: { open: false, pending: false, projectId: 'A', nodes },
      attachTo: document.body,
    })

    await wrapper.setProps({ open: true, initialSourceId: 'A', initialKind: 'part_of' })

    const selects = document.body.querySelectorAll<HTMLSelectElement>('.relationship-dialog select')
    expect(selects[0]?.value).toBe('part_of')
    expect(selects[1]?.value).toBe('A')
    expect(selects[1]?.disabled).toBe(true)
    expect(selects[2]?.value).toBe('')
    wrapper.unmount()
  })
})