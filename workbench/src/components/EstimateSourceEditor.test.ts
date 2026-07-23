import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import type { Estimate } from '../api/types'
import EstimateSourceEditor from './EstimateSourceEditor.vue'

vi.mock('../api/client', () => ({
  api: {
    assessSquiggle: vi.fn().mockResolvedValue({
      assessment: { family: 'PointMass', mean: 0.5, variance: 0, p05: 0.5, p50: 0.5, p95: 0.5, seed: 42, sample_count: 1 },
      effective_distribution: { type: 'point', value: 0.5 },
      predictive_checks: { attempted_draws: 1, valid_draws: 1, invalid_draws: 0, support_violation_draws: 0, support_violation_probability: 0, support_compatible: true, support_requirement: 'values from 0 to 1', representative_outcomes: [{ percentile: 0.1, value: 0.5 }, { percentile: 0.5, value: 0.5 }, { percentile: 0.9, value: 0.5 }] },
    }),
  },
}))

describe('EstimateSourceEditor', () => {
  it('reopens stored Squiggle source', async () => {
    vi.useFakeTimers()
    const estimate = {
      id: 'A', revision: 2,
      distribution: { type: 'point', value: 0.5 },
      source: {
        type: 'squiggle',
        definition: { source: 'pointMass(0.5)', seed: 42, sample_count: 256, target_unit: {} },
        assessment: { family: 'PointMass', mean: 0.5, variance: 0, p05: 0.5, p50: 0.5, p95: 0.5, seed: 42, sample_count: 256 },
      },
    } as Estimate
    const wrapper = mount(EstimateSourceEditor, {
      props: {
        modelValue: { type: 'squiggle', definition: estimate.source.definition },
        existing: estimate,
        projectId: 'A', families: ['point', 'beta'], support: 'probability', expectedUnit: {},
      },
    })
    expect((wrapper.get('[aria-label="Squiggle source"]').element as HTMLTextAreaElement).value).toBe('pointMass(0.5)')
    await vi.advanceTimersByTimeAsync(250)
    await flushPromises()
    expect(wrapper.text()).toContain('Validated · 1 effective samples')
    expect(wrapper.emitted('update:modelValue')!.at(-1)![0]).toMatchObject({
      type: 'squiggle',
      definition: { source: 'pointMass(0.5)', target_unit: {} },
    })
    vi.useRealTimers()
  })

  it('keeps a valid replacement draft instead of restoring the existing source', async () => {
    vi.useFakeTimers()
    const estimate = {
      id: 'A', revision: 2,
      source: {
        type: 'squiggle',
        definition: { source: 'pointMass(0.5)', seed: 42, sample_count: 256, target_unit: {} },
      },
    } as Estimate
    const replacement = {
      type: 'squiggle' as const,
      definition: { source: 'pointMass(0.8)', seed: 42, sample_count: 256, target_unit: {} },
    }
    const wrapper = mount(EstimateSourceEditor, {
      props: {
        modelValue: { type: 'squiggle', definition: estimate.source.definition },
        existing: estimate,
        projectId: 'A', support: 'probability', expectedUnit: {},
      },
    })

    await wrapper.setProps({ modelValue: replacement })
    await vi.advanceTimersByTimeAsync(250)
    await flushPromises()

    expect((wrapper.get('[aria-label="Squiggle source"]').element as HTMLTextAreaElement).value)
      .toBe('pointMass(0.8)')
    expect(wrapper.emitted('update:modelValue')?.at(-1)?.[0]).toMatchObject(replacement)
    vi.useRealTimers()
  })
})