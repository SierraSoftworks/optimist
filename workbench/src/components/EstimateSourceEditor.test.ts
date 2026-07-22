import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import type { Estimate } from '../api/types'
import EstimateSourceEditor from './EstimateSourceEditor.vue'

vi.mock('../api/client', () => ({ api: { assessFermi: vi.fn() } }))

describe('EstimateSourceEditor', () => {
  it('reviews a stored Fermi source and can replace it with a direct distribution', async () => {
    const estimate = {
      id: 'A', revision: 2,
      distribution: { type: 'point', value: 0.5 },
      source: {
        type: 'fermi',
        definition: {
          language: 'optimist_squiggle_v1',
          equation: 'confidence',
          variables: [{ name: 'confidence', estimate: 0.5, unit: '', uncertainty: { type: 'three_point', low: 0.4, high: 0.6 } }],
          formula: { type: 'literal', distribution: { type: 'point', value: 0.5 }, unit: {} },
          monte_carlo: { seed: 42, minimum_samples: 100, maximum_samples: 1000, absolute_tolerance: 0.01, relative_tolerance: 0.01 },
        },
        assessment: {
          compiled: { unit: {}, dependencies: [] },
          report: {
            estimates: [{ mean: 0.5, variance: 0, mean_standard_error: 0, variance_standard_error: 0 }],
            covariance: [[0]],
            diagnostics: {
              seed: 42, attempted_samples: 100, valid_samples: 100,
              invalid_samples: { zero_denominator: 0, non_finite_primitive: 0, non_finite_result: 0 },
              criterion: { seed: 42, minimum_samples: 100, maximum_samples: 1000, absolute_tolerance: 0.01, relative_tolerance: 0.01 },
              status: 'converged',
            },
          },
          recommendation: { status: 'exact', distribution: { type: 'point', value: 0.5 }, interval: { probability: 0.9, lower: 0.5, upper: 0.5 } },
        },
      },
    } as Estimate
    const wrapper = mount(EstimateSourceEditor, {
      props: {
        modelValue: { type: 'fermi', definition: estimate.source!.type === 'fermi' ? estimate.source!.definition : never() },
        existing: estimate,
        projectId: 'A', families: ['point', 'beta'], support: 'probability', expectedUnit: {},
      },
    })
    expect((wrapper.get('[aria-label="Fermi equation"]').element as HTMLInputElement).value).toBe('confidence')
    expect((wrapper.get('[aria-label="Variable 1 name"]').element as HTMLInputElement).value).toBe('confidence')
    expect(wrapper.text()).toContain('Stored effective result')
    await wrapper.get('[aria-label="Estimate source"] button:first-child').trigger('click')
    expect(wrapper.emitted('update:modelValue')!.at(-1)![0]).toMatchObject({ type: 'distribution' })
  })
})

function never(): never {
  throw new Error('unreachable')
}