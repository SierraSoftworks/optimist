import { describe, expect, it } from 'vitest'
import type { ScenarioAnalysis } from '../api/types'
import { referenceCandidate, referenceStates } from './optimizationReference'

type Candidate = ScenarioAnalysis['candidates'][number]

function estimate(mean: number | null) {
  return { mean, variance: 0, mean_standard_error: 0, variance_standard_error: 0 }
}

function candidate(
  intervention: string,
  prerequisites: string[],
  states: number[],
): Candidate {
  return {
    intervention,
    prerequisites,
    blocking_requirements: [],
    synergies: [],
    conflicts: [],
    execution_duration: estimate(0),
    execution_success: estimate(1),
    objectives: [
      {
        outcome: 'A',
        direction: 'minimize',
        importance: 1,
        reachable: true,
        periods_to_effect: 1,
        baseline: estimate(states[0] ?? 0),
        final_state: estimate(states.at(-1) ?? 0),
        improvement: estimate(0),
        trajectory: states.map((state, period) => ({
          period,
          state: estimate(state),
          improvement: estimate(0),
        })),
      },
    ],
    improvement_covariance: [[0]],
    clamped_state_updates: 0,
    undefined_responses: 0,
    diagnostics: {
      seed: 1,
      attempted_samples: 1,
      valid_samples: 1,
      invalid_samples: { non_finite_primitive: 0, non_finite_result: 0 },
      criterion: {
        seed: 1,
        minimum_samples: 1,
        maximum_samples: 1,
        absolute_tolerance: 0,
        relative_tolerance: 0,
      },
      status: 'converged',
    },
  } as unknown as Candidate
}

describe('referenceCandidate', () => {
  /**
   * A candidate that requires a load surge must not be credited with the surge
   * itself, so its reference is the run that executes only what it requires.
   */
  it('finds the run that executes exactly what this candidate requires', () => {
    const surge = candidate('O', [], [1, 8])
    const shedding = candidate('K', ['O'], [1, 4])
    expect(referenceCandidate(shedding, [surge, shedding])).toBe(surge)
  })

  it('has no reference when the candidate requires nothing', () => {
    const surge = candidate('O', [], [1, 8])
    expect(referenceCandidate(surge, [surge])).toBeNull()
  })

  it('has no reference when the prerequisites are not offered as a candidate', () => {
    const shedding = candidate('K', ['O'], [1, 4])
    expect(referenceCandidate(shedding, [shedding])).toBeNull()
  })

  it('does not accept a run that executes more than the prerequisites', () => {
    const other = candidate('L', ['O'], [1, 6])
    const shedding = candidate('K', ['O'], [1, 4])
    expect(referenceCandidate(shedding, [other, shedding])).toBeNull()
  })
})

describe('referenceStates', () => {
  it('takes the states of the reference run when there is one', () => {
    const surge = candidate('O', [], [1, 8, 20])
    const shedding = candidate('K', ['O'], [1, 4, 5])
    expect(referenceStates(shedding, 'A', surge, 3)).toEqual([1, 8, 20])
  })

  it('holds the resting level when nothing was projected to compare against', () => {
    const surge = candidate('O', [], [3, 8, 20])
    expect(referenceStates(surge, 'A', null, 3)).toEqual([3, 3, 3])
  })
})
