import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import type { Estimate } from '../api/types'
import EstimateSourceEditor from './EstimateSourceEditor.vue'

vi.mock('../api/client', () => ({
  api: {
    assessSquiggle: vi.fn().mockResolvedValue({
      assessment: { family: 'PointMass', mean: 0.5, variance: 0, p05: 0.5, p50: 0.5, p95: 0.5, seed: 42, sample_count: 1 },
      effective_distribution: { type: 'point', value: 0.5 },
    }),
  },
}))

describe('EstimateSourceEditor', () => {
  it('translates a stored Fermi result into backend-assessed Squiggle source', async () => {
    vi.useFakeTimers()
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
    expect((wrapper.get('[aria-label="Squiggle source"]').element as HTMLTextAreaElement).value).toBe('pointMass(0.5)')
    expect(wrapper.text()).toContain('legacy fermi estimate')
    await vi.advanceTimersByTimeAsync(250)
    await flushPromises()
    expect(wrapper.text()).toContain('PointMass')
    expect(wrapper.emitted('update:modelValue')!.at(-1)![0]).toMatchObject({
      type: 'squiggle',
      definition: { source: 'pointMass(0.5)', target_unit: {} },
    })
    vi.useRealTimers()
  })
})

function never(): never {
  throw new Error('unreachable')
}