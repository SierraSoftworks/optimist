import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../api/client'
import type { GraphEdge, GraphNode } from '../api/types'
import EditStateEstimateDialog from './EditStateEstimateDialog.vue'

const node = {
  id: 'B', revision: 0, name: 'flow', normalized_name: 'flow', title: 'Flow',
  description: '', aliases: [], metadata: {},
  payload: { kind: 'factor', properties: { current: null, desired: null, controllable: false, evidence: [] } },
} as GraphNode
const edge = {
  source: 'A', source_kind: 'metric', destination: 'B', destination_kind: 'factor',
  revision: 2, description: '', metadata: {},
  payload: {
    kind: 'measures',
    properties: {
      polarity: 'lower_is_better',
      calibration: { type: 'linear', state_zero: 20, state_one: 5 },
      observations: [
        { id: 0, revision: 0, value: 16, unit: 'days', observed_at: '2026-07-01T00:00:00Z', source: 'dashboard', measurement_standard_deviation: null, supersedes: null },
        { id: 1, revision: 0, value: 12.5, unit: 'days', observed_at: '2026-07-01T00:00:00Z', source: 'dashboard', measurement_standard_deviation: null, supersedes: 0 },
      ],
    },
  },
} as GraphEdge

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

  it('offers the corrected calibrated reading and records adoption provenance', async () => {
    const wrapper = mount(EditStateEstimateDialog, {
      props: { open: true, pending: false, node, projectId: 'A', edges: [edge] },
      global: { stubs: { Teleport: true } },
    })
    expect(wrapper.text()).toContain('12.5 days → 0.500')
    expect(wrapper.text()).not.toContain('16 days →')
    await wrapper.get('.calibrated-evidence .secondary-button').trigger('click')
    expect((wrapper.get('[aria-label="Squiggle source"]').element as HTMLTextAreaElement).value).toBe('pointMass(0.5)')
    expect((wrapper.get('textarea[placeholder="One source or elicitation note per line"]').element as HTMLTextAreaElement).value).toContain('Calibrated observation #1')
  })

  it('captures distinct uncertainty sources without combining them', async () => {
    const wrapper = mount(EditStateEstimateDialog, {
      props: { open: true, pending: false, node, projectId: 'A', edges: [] },
      global: { stubs: { Teleport: true } },
    })
    await wrapper.findAll('.uncertainty-editor textarea')[0]!.setValue('Limited calibration evidence')
    await wrapper.findAll('.uncertainty-editor textarea')[1]!.setValue('Daily process variation')
    await wrapper.findAll('.uncertainty-editor textarea')[2]!.setValue('Survey sampling error')
    await wrapper.get('form').trigger('submit')

    expect(wrapper.emitted('submit')?.[0]?.[0]).toMatchObject({
      uncertainty: {
        epistemic: 'Limited calibration evidence',
        process: 'Daily process variation',
        measurement: 'Survey sampling error',
      },
    })
  })

  it('edits a bounded metric directly in its native unit without offering Fermi', async () => {
    const metric = {
      id: 'A', revision: 0, name: 'lead_time', normalized_name: 'lead_time', title: 'Lead time',
      description: '', aliases: [], metadata: {},
      payload: {
        kind: 'metric',
        properties: {
          unit: 'days', dimension: { day: 1 }, aggregation: 'p95 weekly',
          support: { type: 'bounded', lower: 0, upper: 30 }, current: null,
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
          unit: 'days', dimension: { day: 1 }, aggregation: null,
          support: { type: 'bounded', lower: 0, upper: 30 }, current: null,
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