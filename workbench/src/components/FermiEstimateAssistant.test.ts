import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { api } from '../api/client'
import * as squigglePreview from '../domain/squigglePreview'
import FermiEstimateAssistant from './FermiEstimateAssistant.vue'

afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
})

describe('FermiEstimateAssistant', () => {
  it('shows a live Squiggle interval as the equation changes', async () => {
    vi.useFakeTimers()
    vi.spyOn(squigglePreview, 'evaluateSquigglePreview').mockResolvedValue({
      mean: 8, standardDeviation: 2, p05: 4, p25: 6, p50: 8, p75: 9, p95: 12,
      supportViolationProbability: 0.12, samples: 4_000, executionMilliseconds: 12,
    })
    const wrapper = mount(FermiEstimateAssistant, {
      props: { projectId: 'A', support: 'non_negative', expectedUnit: {}, modelValue: null },
    })

    await wrapper.get('.fermi-toggle').trigger('click')
    expect(wrapper.text()).toContain('Evaluating the current uncertainty model')
    await vi.advanceTimersByTimeAsync(180)
    await flushPromises()

    expect(wrapper.get('.squiggle-preview').text()).toContain('90% interval')
    expect(wrapper.get('.squiggle-preview').text()).toContain('4,000 deterministic samples')
    expect(wrapper.get('.squiggle-warning').text()).toContain('12% of predicted values are negative')
    expect(wrapper.get('.squiggle-track').attributes('aria-label')).toContain('median')
  })

  it('assesses PERT components and explicitly applies the recommendation', async () => {
    const assess = vi.spyOn(api, 'assessFermi').mockResolvedValue({
      compiled: { unit: {}, dependencies: [] },
      report: {
        estimates: [{ mean: 0.58, variance: 0.02, mean_standard_error: 0.001, variance_standard_error: 0.001 }],
        covariance: [[0.02]],
        diagnostics: {
          seed: 42, attempted_samples: 2000, valid_samples: 2000,
          invalid_samples: { zero_denominator: 0, non_finite_primitive: 0, non_finite_result: 0 },
          criterion: { seed: 42, minimum_samples: 2000, maximum_samples: 20000, absolute_tolerance: 0.001, relative_tolerance: 0.01 },
          status: 'converged',
        },
      },
      recommendation: {
        status: 'moment_matched',
        distribution: { type: 'beta', alpha: 6, beta: 4 },
        interval: { probability: 0.9, lower: 0.3, upper: 0.8 },
        warning: 'Mean and variance only.',
      },
    })
    const wrapper = mount(FermiEstimateAssistant, {
      props: { projectId: 'A', support: 'probability', expectedUnit: {}, modelValue: null },
    })
    await wrapper.get('.fermi-toggle').trigger('click')
    await wrapper.get('.fermi-actions button:last-child').trigger('click')
    await flushPromises()

    expect(assess).toHaveBeenCalledWith('A', expect.objectContaining({
      support: 'probability', expected_unit: {}, formula: expect.objectContaining({ type: 'bounded' }),
    }))
    expect(wrapper.text()).toContain('2,000 samples · converged')
    await wrapper.get('.fermi-result .primary-button').trigger('click')
    expect(wrapper.emitted('update:modelValue')![0]![0]).toMatchObject({
      equation: 'x * y',
      formula: { type: 'bounded' },
      variables: [{ name: 'x' }, { name: 'y' }],
    })
  })

  it('shows unit validation failures without applying a distribution', async () => {
    vi.spyOn(api, 'assessFermi').mockRejectedValue(new Error('decomposition unit days does not match target unit duration'))
    const wrapper = mount(FermiEstimateAssistant, {
      props: { projectId: 'A', support: 'non_negative', expectedUnit: { duration: 1 }, modelValue: null },
    })
    await wrapper.get('.fermi-toggle').trigger('click')
    await wrapper.get('.fermi-actions button:last-child').trigger('click')
    await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toContain('does not match target unit')
    expect(wrapper.emitted('update:modelValue')).toBeUndefined()
  })

  it('builds the piano equation and highlights its unresolved subject dimension', async () => {
    const assess = vi.spyOn(api, 'assessFermi').mockResolvedValue({
      compiled: { unit: { piano: 1, day: -1 }, dependencies: [] },
      report: {
        estimates: [{ mean: 138.9, variance: 400, mean_standard_error: 0.2, variance_standard_error: 5 }],
        covariance: [[400]],
        diagnostics: {
          seed: 42, attempted_samples: 2000, valid_samples: 2000,
          invalid_samples: { zero_denominator: 0, non_finite_primitive: 0, non_finite_result: 0 },
          criterion: { seed: 42, minimum_samples: 2000, maximum_samples: 20000, absolute_tolerance: 0.001, relative_tolerance: 0.01 },
          status: 'converged',
        },
      },
      recommendation: { status: 'moment_matched', distribution: { type: 'log_normal', location: 4.8, scale: 0.3 }, interval: { probability: 0.9, lower: 80, upper: 220 }, warning: 'Approximation' },
    })
    const wrapper = mount(FermiEstimateAssistant, {
      props: { projectId: 'A', support: 'non_negative', expectedUnit: { piano: 1, day: -1 }, modelValue: null },
    })
    await wrapper.get('.fermi-toggle').trigger('click')
    await wrapper.get('[aria-label="Fermi equation"]').setValue('people / people_per_household / households_per_piano / days_per_tuning * pianos_per_tuning')
    await setVariable(wrapper, 1, 'people', '1.5M', 'people')
    await setVariable(wrapper, 2, 'people_per_household', '3', 'people/household')
    for (let count = 0; count < 3; count += 1) await wrapper.get('.fermi-actions button:first-child').trigger('click')
    await setVariable(wrapper, 3, 'households_per_piano', '20', 'households/piano')
    await setVariable(wrapper, 4, 'days_per_tuning', '180', 'days/tuning')
    await setVariable(wrapper, 5, 'pianos_per_tuning', '1', 'pianos/tuning')

    expect(wrapper.get('.fermi-equation-status').text()).toContain('138.889')
    expect(wrapper.get('.fermi-equation-status').text()).toContain('piano^2/day')
    expect(wrapper.get('.fermi-equation-status').text()).toContain('Unresolved dimension: piano')
    expect(wrapper.get('.fermi-actions button:last-child').attributes('disabled')).toBeDefined()

    await wrapper.get('[aria-label="Variable 4 unit"]').setValue('piano*days/tuning')
    expect(wrapper.get('.fermi-equation-status').text()).toContain('Derived unitpiano/day')
    await wrapper.get('.fermi-actions button:last-child').trigger('click')
    await flushPromises()
    expect(assess).toHaveBeenCalledWith('A', expect.objectContaining({
      expected_unit: { piano: 1, day: -1 },
      formula: expect.objectContaining({ type: 'product' }),
    }))
  })
})

async function setVariable(
  wrapper: ReturnType<typeof mount>,
  index: number,
  name: string,
  estimate: string,
  unit: string,
) {
  await wrapper.get(`[aria-label="Variable ${index} name"]`).setValue(name)
  await wrapper.get(`[aria-label="Variable ${index} estimate"]`).setValue(estimate)
  await wrapper.get(`[aria-label="Variable ${index} estimate"]`).trigger('change')
  await wrapper.get(`[aria-label="Variable ${index} unit"]`).setValue(unit)
}