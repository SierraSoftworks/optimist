import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import { nextTick } from 'vue'
import CommandBar from './CommandBar.vue'
import type { GraphNode } from '../api/types'

const nodes = [{ id: 'A', name: 'flow', title: 'Flow', payload: { kind: 'factor' } }] as GraphNode[]

describe('CommandBar', () => {
  it('previews and emits a deterministic typed command', async () => {
    const wrapper = mount(CommandBar, {
      props: { open: true, pending: false, nodes, edges: [] },
      attachTo: document.body,
    })
    const input = document.body.querySelector<HTMLInputElement>('[aria-label="Command"]')!
    input.value = 'select A'
    input.dispatchEvent(new Event('input', { bubbles: true }))
    await nextTick()
    expect(document.body.textContent).toContain('Inspect node')
    document.body.querySelector<HTMLFormElement>('.command-bar')!
      .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }))
    expect(wrapper.emitted('apply')?.[0]?.[0]).toMatchObject({ type: 'select_node', node: { id: 'A' } })
    wrapper.unmount()
  })
})