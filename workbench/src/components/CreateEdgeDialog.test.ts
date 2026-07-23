import { flushPromises, mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { api } from '../api/client'
import type { GraphNode } from '../api/types'
import CreateEdgeDialog from './CreateEdgeDialog.vue'

vi.mock('../api/client', () => ({
  api: {
    assessSquiggle: vi.fn().mockResolvedValue({
      assessment: { family: 'PointMass', mean: 1, variance: 0, p05: 1, p50: 1, p95: 1, seed: 42, sample_count: 2_048 },
      effective_distribution: { type: 'point', value: 1 },
      predictive_checks: { attempted_draws: 2_048, valid_draws: 2_048, invalid_draws: 0, support_violation_draws: 0, support_violation_probability: 0, representative_outcomes: [] },
    }),
  },
}))

const nodes = [
  { id: 'A', title: 'Source factor', payload: { kind: 'factor' } },
  { id: 'B', title: 'Destination factor', payload: { kind: 'factor' } },
] as GraphNode[]

const causalNodes = nodes.map((node) => ({
  ...node,
  native_state: {
    quantity: { unit: 'score', dimension: { score: 1 }, aggregation: null },
    current: null,
    forecast: null,
  },
})) as GraphNode[]

afterEach(() => {
  vi.useRealTimers()
  vi.mocked(api.assessSquiggle).mockClear()
})

describe('CreateEdgeDialog', () => {
  it('does not evaluate a response before the target unit is available', async () => {
    vi.useFakeTimers()
    const wrapper = mount(CreateEdgeDialog, {
      props: {
        open: true, pending: false, projectId: 'A', nodes: causalNodes,
        sourceId: 'A', destinationId: '', kind: 'contributes', sourceLocked: true,
      },
      attachTo: document.body,
    })

    await vi.advanceTimersByTimeAsync(300)
    await flushPromises()
    expect(api.assessSquiggle).not.toHaveBeenCalled()

    await vi.advanceTimersByTimeAsync(300)
    await flushPromises()
    expect(api.assessSquiggle).not.toHaveBeenCalled()
    wrapper.unmount()
  })

  it('prefills a relationship kind and source when opened from a node action', async () => {
    const wrapper = mount(CreateEdgeDialog, {
      props: {
        open: false, pending: false, projectId: 'A', nodes,
        sourceId: '', destinationId: '', kind: 'contributes', sourceLocked: false,
      },
      attachTo: document.body,
    })

    await wrapper.setProps({ open: true, sourceId: 'A', kind: 'part_of', sourceLocked: true })

    const selects = document.body.querySelectorAll<HTMLSelectElement>('.relationship-dialog select')
    const source = document.body.querySelector<HTMLInputElement>('input[name="relationship-source"][value="A"]')!
    expect(selects).toHaveLength(1)
    expect(selects[0]?.value).toBe('part_of')
    expect(source.checked).toBe(true)
    expect(source.disabled).toBe(true)
    expect(document.body.querySelector('input[name="relationship-destination"]:checked')).toBeNull()
    wrapper.unmount()
  })

  it('retains selected endpoints through reactive refreshes and assessment', async () => {
    vi.useFakeTimers()
    const wrapper = mount(CreateEdgeDialog, {
      props: {
        open: true, pending: false, projectId: 'A', nodes: causalNodes,
        sourceId: '', destinationId: '', kind: 'contributes', sourceLocked: false,
      },
      attachTo: document.body,
    })
    const source = document.body.querySelector<HTMLInputElement>('input[name="relationship-source"][value="A"]')!
    source.click()
    await nextTick()
    expect(wrapper.emitted('draftChange')?.at(-1)).toEqual([{
      sourceId: 'A', destinationId: '', kind: 'contributes',
    }])
    await wrapper.setProps({ sourceId: 'A', destinationId: '' })
    const destination = document.body.querySelector<HTMLInputElement>('input[name="relationship-destination"][value="B"]')!
    destination.click()
    await nextTick()
    expect(wrapper.emitted('draftChange')?.at(-1)).toEqual([{
      sourceId: 'A', destinationId: 'B', kind: 'contributes',
    }])
    await wrapper.setProps({ destinationId: 'B' })

    for (let refresh = 0; refresh < 3; refresh += 1) {
      await wrapper.setProps({
        pending: refresh % 2 === 0,
        nodes: causalNodes.map((node) => ({ ...node })),
      })
    }
    await vi.advanceTimersByTimeAsync(300)
    await flushPromises()

    expect(source.checked).toBe(true)
    expect(destination.checked).toBe(true)
    expect(document.body.textContent).toContain('Validated')
    wrapper.unmount()
  })
})