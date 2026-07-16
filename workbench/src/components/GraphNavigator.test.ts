import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import type { GraphNode } from '../api/types'
import GraphNavigator from './GraphNavigator.vue'

function node(id: string, title: string, kind: 'factor' | 'outcome' = 'factor'): GraphNode {
  return {
    id,
    revision: 0,
    name: title.toLocaleLowerCase().replaceAll(' ', '_'),
    normalized_name: title.toLocaleLowerCase().replaceAll(' ', '_'),
    title,
    description: '',
    aliases: [],
    metadata: {},
    payload: kind === 'factor'
      ? { kind, properties: { current: null, desired: null, controllable: false, evidence: [] } }
      : { kind, properties: { direction: 'maximize', current: null, desired: null, evidence: [] } },
  }
}

const nodes = [node('A', 'Fast feedback'), node('B', 'Learning rate'), node('C', 'Reliability', 'outcome')]

describe('GraphNavigator', () => {
  it('uses roving focus and arrow keys to synchronize selection', async () => {
    const wrapper = mount(GraphNavigator, { props: { nodes, selectedNodeId: 'A' }, attachTo: document.body })
    const buttons = wrapper.findAll('.node-outline button')
    expect(buttons.map((button) => button.attributes('tabindex'))).toEqual(['0', '-1', '-1'])

    await buttons[0]!.trigger('keydown', { key: 'ArrowDown' })
    expect(wrapper.emitted('select')!.at(-1)).toEqual(['B'])
    await wrapper.setProps({ selectedNodeId: 'B' })
    expect(document.activeElement?.textContent).toContain('Learning rate')

    await wrapper.findAll('.node-outline button')[1]!.trigger('keydown', { key: 'End' })
    expect(wrapper.emitted('select')!.at(-1)).toEqual(['C'])
    wrapper.unmount()
  })

  it('renders a semantic table and preserves selection across views', async () => {
    const wrapper = mount(GraphNavigator, { props: { nodes, selectedNodeId: 'B' } })
    await wrapper.get('button[aria-label="Table view"]').trigger('click')
    expect(wrapper.get('table caption').text()).toBe('Visible graph nodes')
    expect(wrapper.findAll('thead th').map((header) => header.text())).toEqual(['ID', 'Title', 'Kind'])
    expect(wrapper.get('tbody tr.selected').text()).toContain('Learning rate')
    expect(wrapper.get('tbody button[aria-current="true"]').text()).toBe('Learning rate')
  })
})
