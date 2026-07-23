import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { api } from '../api/client'
import SquiggleEstimateEditor from './SquiggleEstimateEditor.vue'

describe('SquiggleEstimateEditor predictive checks', () => {
  afterEach(() => {
    vi.useRealTimers()
    vi.restoreAllMocks()
  })

  it('shows compact support validation failures before adoption', async () => {
    vi.useFakeTimers()
    vi.spyOn(api, 'assessSquiggle').mockResolvedValue({
      assessment: {
        family: 'Normal', mean: 0.5, variance: 0.16,
        p05: -0.2, p50: 0.5, p95: 1.2, seed: 42, sample_count: 5,
      },
      effective_distribution: { type: 'empirical', samples: [-0.2, 0.1, 0.5, 0.9, 1.2] },
      predictive_checks: {
        attempted_draws: 5,
        valid_draws: 5,
        invalid_draws: 0,
        support_violation_draws: 2,
        support_violation_probability: 0.4,
        representative_outcomes: [
          { percentile: 0.1, value: -0.1 },
          { percentile: 0.5, value: 0.5 },
          { percentile: 0.9, value: 1.1 },
        ],
      },
    })
    const wrapper = mount(SquiggleEstimateEditor, {
      props: {
        projectId: 'A',
        modelValue: { source: 'normal(0.5, 0.4)', seed: 42, sample_count: 256, target_unit: {} },
        support: 'probability',
        expectedUnit: {},
      },
    })

    await vi.advanceTimersByTimeAsync(250)
    await flushPromises()

    expect(api.assessSquiggle).toHaveBeenCalledWith('A', expect.any(Object), 'probability')
    expect(wrapper.text()).toContain('Validation issue')
    expect(wrapper.text()).toContain('40.00%')
    expect(wrapper.text()).toContain('2 retained draws fall outside this estimate slot')
    expect(wrapper.text()).not.toContain('P10')
    expect(wrapper.emitted('validity')?.at(-1)).toEqual([false])
  })
})
