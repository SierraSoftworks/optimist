import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../api/client'
import type { GraphNode } from '../api/types'
import EditStateEstimateDialog from './EditStateEstimateDialog.vue'

const node = {
  id: 'B', revision: 0, name: 'flow', normalized_name: 'flow', title: 'Flow',
  description: '', aliases: [], metadata: {},
  native_state: {
    quantity: { unit: 'state', dimension: {}, aggregation: null, support: { type: 'bounded', lower: 0, upper: 1 } },
    current: null,
    forecast: null,
  },
  payload: { kind: 'factor', properties: { controllable: false, evidence: [] } },
} as GraphNode
describe('EditStateEstimateDialog', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.spyOn(api, 'assessSquiggle').mockResolvedValue({
      assessment: { family: 'PointMass', mean: 0.5, variance: 0, p05: 0.5, p50: 0.5, p95: 0.5, seed: 42, sample_count: 1 },
      effective_distribution: { type: 'point', value: 0.5 },
      predictive_checks: { attempted_draws: 1, valid_draws: 1, invalid_draws: 0, support_violation_draws: 0, support_violation_probability: 0, representative_outcomes: [{ percentile: 0.1, value: 0.5 }, { percentile: 0.5, value: 0.5 }, { percentile: 0.9, value: 0.5 }] },
    })
  })
  afterEach(() => {
    vi.useRealTimers()
    vi.restoreAllMocks()
  })

  it('preserves existing metadata without displaying metadata controls', async () => {
    const existing = {
      ...node,
      native_state: {
        ...node.native_state!,
        current: {
          id: 'A', revision: 0,
          distribution: { type: 'point' as const, value: 0.5 },
          source: {
            type: 'squiggle' as const,
            definition: { source: 'pointMass(0.5)', seed: 42, sample_count: 256, target_unit: {} },
          },
          provenance: ['existing source'],
          uncertainty: { epistemic: 'existing assumption' },
        },
      },
    } as GraphNode
    const wrapper = mount(EditStateEstimateDialog, {
      props: { open: true, pending: false, node: existing, projectId: 'A', edges: [] },
      global: { stubs: { Teleport: true } },
    })
    await wrapper.get('form').trigger('submit')

    expect(wrapper.emitted('submit')?.[0]?.[0]).toMatchObject({
      provenance: ['existing source'],
      uncertainty: { epistemic: 'existing assumption' },
    })
    expect(wrapper.find('.uncertainty-editor').exists()).toBe(false)
  })

  it('edits a bounded metric directly in its native unit with Squiggle', async () => {
    const metric = {
      id: 'A', revision: 0, name: 'lead_time', normalized_name: 'lead_time', title: 'Lead time',
      description: '', aliases: [], metadata: {},
      payload: {
        kind: 'metric',
        properties: {
          quantity: {
            unit: 'days', dimension: { day: 1 }, aggregation: 'p95 weekly',
            support: { type: 'bounded', lower: 0, upper: 30 },
          },
          current: null,
        },
      },
    } as GraphNode
    const wrapper = mount(EditStateEstimateDialog, {
      props: { open: true, pending: false, node: metric, projectId: 'A', edges: [] },
      global: { stubs: { Teleport: true } },
    })

    expect(wrapper.text()).toContain('Set quantity estimate')
    expect(wrapper.find('[aria-label="Estimate source"]').exists()).toBe(false)
    const source = wrapper.get('[aria-label="Squiggle source"]')
    expect((source.element as HTMLTextAreaElement).value).toBe('beta(2, 2) * 30 + 0')
    await vi.advanceTimersByTimeAsync(250)
    await flushPromises()
    await wrapper.get('form').trigger('submit')
    expect(wrapper.emitted('submit')?.[0]?.[0]).toMatchObject({
      slot: 'current',
      source: { type: 'squiggle', definition: { source: 'beta(2, 2) * 30 + 0' } },
    })
  })

  it('shows the canonical target unit while authoring a bounded metric', async () => {
    const metric = {
      id: 'A', revision: 0, name: 'lead_time', normalized_name: 'lead_time', title: 'Lead time',
      description: '', aliases: [], metadata: {},
      payload: {
        kind: 'metric',
        properties: {
          quantity: {
            unit: 'days', dimension: { day: 1 }, aggregation: null,
            support: { type: 'bounded', lower: 0, upper: 30 },
          },
          current: null,
        },
      },
    } as GraphNode
    const wrapper = mount(EditStateEstimateDialog, {
      props: { open: true, pending: false, node: metric, projectId: 'A', edges: [] },
      global: { stubs: { Teleport: true } },
    })

    expect(wrapper.find('[aria-label="Squiggle source"]').exists()).toBe(true)
    expect(wrapper.text()).toContain('day')
  })

  it('authors native factor forecasts with owner support and units', () => {
    const native = {
      ...node,
      native_state: {
        quantity: {
          unit: 'days', dimension: { day: 1 }, aggregation: null,
          support: { type: 'bounded' as const, lower: 0, upper: 30 },
          operational_definition: 'Elapsed lead time',
        },
        current: null,
        forecast: null,
      },
    } as GraphNode
    const wrapper = mount(EditStateEstimateDialog, {
      props: { open: true, pending: false, node: native, projectId: 'A', edges: [] },
      global: { stubs: { Teleport: true } },
    })

    expect(wrapper.text()).toContain('Forecast')
    expect((wrapper.get('[aria-label="Squiggle source"]').element as HTMLTextAreaElement).value)
      .toBe('beta(2, 2) * 30 + 0')
    expect(wrapper.text()).toContain('day')
  })
})