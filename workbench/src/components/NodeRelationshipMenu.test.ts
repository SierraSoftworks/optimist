import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import type { GraphNode } from '../api/types'
import NodeRelationshipMenu from './NodeRelationshipMenu.vue'

const nodes = [
  { id: 'A', title: 'Source factor', payload: { kind: 'factor' } },
  { id: 'B', title: 'Destination factor', payload: { kind: 'factor' } },
] as GraphNode[]

describe('NodeRelationshipMenu', () => {
  it('offers only relationship kinds compatible with the source node', async () => {
    const wrapper = mount(NodeRelationshipMenu, {
      props: { open: true, source: nodes[0]!, nodes, x: 40, y: 60 },
      attachTo: document.body,
    })

    const menu = document.body.querySelector<HTMLElement>('[role="menu"]')!
    expect(menu.getAttribute('aria-label')).toBe('Add relationship from Source factor')
    expect(menu.textContent).toContain('Contributes')
    expect(menu.textContent).toContain('Requires')
    expect(menu.textContent).toContain('Part of')
    expect(menu.textContent).toContain('Blocks')
    expect(menu.textContent).not.toContain('Measures')

    const partOf = Array.from(menu.querySelectorAll<HTMLButtonElement>('button'))
      .find((button) => button.textContent?.trim() === 'Part of')!
    partOf.click()
    expect(wrapper.emitted('select')![0]).toEqual(['part_of'])
    wrapper.unmount()
  })
})