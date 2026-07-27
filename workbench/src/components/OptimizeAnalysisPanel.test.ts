import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import type { GraphNode, Scenario, ScenarioAnalysis } from '../api/types'
import OptimizeAnalysisPanel from './OptimizeAnalysisPanel.vue'

const nodes: GraphNode[] = [
  {
    id: 'A', revision: 0, name: 'reliability', normalized_name: 'reliability', title: 'Reliability',
    description: '', aliases: [], metadata: {},
    payload: { kind: 'outcome', properties: { direction: 'maximize', evidence: [] } },
  },
  {
    id: 'B', revision: 0, name: 'automate', normalized_name: 'automate', title: 'Automate',
    description: '', aliases: [], metadata: {},
    payload: { kind: 'intervention', properties: { costs: [], duration: null, probability_of_success: null, acceptance_criteria: [] } },
  },
]

const config = {
  seed: 42, minimum_samples: 100, maximum_samples: 1000,
  absolute_tolerance: 0.01, relative_tolerance: 0.01,
}
const scenario: Scenario = {
  id: 'A', revision: 0, name: 'delivery', title: 'Delivery', rationale: '',
  objectives: [{ outcome_id: 'A', direction: 'maximize', importance: 1 }],
  planning_horizon: 12, budgets: [], candidate_interventions: ['B'], monte_carlo: config,
}
const estimate = { mean: 0.12, variance: 0.02, mean_standard_error: 0.004, variance_standard_error: 0.003 }
const baselineEstimate = { ...estimate, mean: 0.5 }
const finalEstimate = { ...estimate, mean: 0.62 }
const trajectory = [
  { period: 0, state: { ...estimate, mean: 0.5 }, improvement: { ...estimate, mean: 0 } },
  { period: 12, state: { ...estimate, mean: 0.62 }, improvement: estimate },
]
const analysis: ScenarioAnalysis = {
  revision: { project: 'A', graph_revision: 5, scenario: ['A', 0], dependence_revision: null },
  planning_horizon: 12,
  candidates: [{
    intervention: 'B',
    prerequisites: [], blocking_requirements: [], synergies: [], conflicts: [],
    execution_duration: { ...estimate, mean: 3 },
    execution_success: { ...estimate, mean: 0.8 },
    objectives: [{
      outcome: 'A', direction: 'maximize', importance: 1, reachable: true, periods_to_effect: 2,
      baseline: baselineEstimate, final_state: finalEstimate, improvement: estimate, trajectory,
    }],
    improvement_covariance: [[0.02]],
    clamped_state_updates: 3,
    undefined_responses: 0,
    diagnostics: {
      seed: 42, attempted_samples: 120, valid_samples: 118,
      invalid_samples: { non_finite_primitive: 2, non_finite_result: 0 },
      criterion: config, status: 'converged',
    },
  }],
  feedback_loops: [],
}

describe('OptimizeAnalysisPanel', () => {
  it('offers scenario creation without inventing a comparison', () => {
    const wrapper = mount(OptimizeAnalysisPanel, {
      props: {
        scenarios: [], selectedScenarioId: null, analysis: undefined,
        pending: false, error: null, nodes, selectedCandidateId: null,
      },
    })
    expect(wrapper.text()).toContain('No scenarios yet')
    expect(wrapper.get('button.primary-button').text()).toContain('Create scenario')
  })

  it('renders objective projections and numerical diagnostics without a scalar rank', async () => {
    const wrapper = mount(OptimizeAnalysisPanel, {
      props: {
        scenarios: [scenario], selectedScenarioId: 'A', analysis,
        pending: false, error: null, nodes, selectedCandidateId: null,
      },
    })
    expect(wrapper.text()).toContain('Automate')
    expect(wrapper.text()).toContain('24.0% better')
    expect(wrapper.text()).toContain('118 / 120')
    expect(wrapper.text()).toContain('Without')
    expect(wrapper.text()).toContain('Plan success')
    expect(
      wrapper
        .get('figure[aria-label="Reliability over time, with and without this intervention"]')
        .text(),
    ).toContain('resting level')
    await wrapper.get('.candidate-summary').trigger('click')
    expect(wrapper.emitted('selectCandidate')![0]).toEqual(['B', ['B', 'A']])
  })

  it('colors a decreasing minimize objective as positive impact', () => {
    const minimizeTrajectory = [
      { period: 0, state: { ...estimate, mean: 0.7 }, improvement: { ...estimate, mean: 0 } },
      { period: 12, state: { ...estimate, mean: 0.5 }, improvement: { ...estimate, mean: 0.2 } },
    ]
    const minimizeAnalysis: ScenarioAnalysis = {
      ...analysis,
      candidates: [{
        ...analysis.candidates[0]!,
        objectives: [{
          ...analysis.candidates[0]!.objectives[0]!,
          direction: 'minimize',
          baseline: { ...estimate, mean: 0.7 },
          final_state: { ...estimate, mean: 0.5 },
          improvement: { ...estimate, mean: 0.2 },
          trajectory: minimizeTrajectory,
        }],
      }],
    }
    const wrapper = mount(OptimizeAnalysisPanel, {
      props: {
        scenarios: [scenario], selectedScenarioId: 'A', analysis: minimizeAnalysis,
        pending: false, error: null, nodes, selectedCandidateId: null,
      },
    })

    expect(wrapper.get('.relative-impact').attributes('data-impact')).toBe('positive')
    expect(wrapper.get('.relative-impact').text()).toBe('28.6% better')
    expect(wrapper.get('.trajectory').attributes('data-impact')).toBe('positive')
    expect(wrapper.get('.trajectory').text()).toContain('minimize')
  })

  /**
   * A candidate that requires a load surge runs under it either way, so crediting
   * it with the surge is what produced the six-digit percentages this view used
   * to report. The comparison has to be against the surge alone.
   */
  it('reads a candidate against the run of its prerequisites, not against rest', () => {
    const states = (mean: number) => ({ ...estimate, mean })
    const objective = (settled: number) => ({
      outcome: 'A', direction: 'maximize' as const, importance: 1, reachable: true,
      periods_to_effect: 2,
      baseline: states(0.5), final_state: states(settled), improvement: estimate,
      trajectory: [
        { period: 0, state: states(0.5), improvement: states(0) },
        { period: 12, state: states(settled), improvement: states(0) },
      ],
    })
    const surge = {
      ...analysis.candidates[0]!,
      intervention: 'C',
      prerequisites: [],
      objectives: [objective(0.2)],
    }
    const mitigation = {
      ...analysis.candidates[0]!,
      intervention: 'B',
      prerequisites: ['C'],
      objectives: [objective(0.4)],
    }
    const wrapper = mount(OptimizeAnalysisPanel, {
      props: {
        scenarios: [scenario], selectedScenarioId: 'A',
        analysis: { ...analysis, candidates: [mitigation, surge] },
        pending: false, error: null, nodes, selectedCandidateId: null,
      },
    })

    // The rail carries every candidate's change so they can be compared at once.
    const summaries = wrapper.findAll('.summary-change')
    // Against the surge's 0.2 the mitigation's 0.4 doubles the outcome; against
    // the resting 0.5 it would have read as a loss.
    expect(summaries[0]!.text()).toBe('100.0% better')
    expect(summaries[0]!.attributes('data-impact')).toBe('positive')
    // The surge itself requires nothing, so it is still read against rest.
    expect(summaries[1]!.text()).toBe('60.0% worse')
    expect(wrapper.findAll('.trajectory')[0]!.text()).toContain('prerequisites alone')
  })

  /**
   * Debugging a model means reading every state, including the ones no objective
   * names. A state that never moves is called out because that is usually the
   * one at fault when a projection looks wrong.
   */
  it('traces every propagated state when the projection carries them', async () => {
    const point = (mean: number) => ({ ...estimate, mean })
    const withStates: ScenarioAnalysis = {
      ...analysis,
      candidates: [{
        ...analysis.candidates[0]!,
        states: [
          { state: 'A', points: [point(0.5), point(0.62)] },
          { state: 'C', points: [point(2), point(2)] },
        ],
      }],
    }
    const wrapper = mount(OptimizeAnalysisPanel, {
      props: {
        scenarios: [scenario], selectedScenarioId: 'A', analysis: withStates,
        pending: false, error: null, nodes, selectedCandidateId: null,
      },
    })

    expect(wrapper.get('details.state-traces summary').text()).toContain('2')
    const traces = wrapper.findAll('figure.state-trace')
    expect(traces).toHaveLength(2)
    expect(traces[0]!.attributes('data-inert')).toBe('false')
    expect(traces[1]!.attributes('data-inert')).toBe('true')
  })

  it('leaves the state traces out when the projection does not carry them', () => {
    const wrapper = mount(OptimizeAnalysisPanel, {
      props: {
        scenarios: [scenario], selectedScenarioId: 'A', analysis,
        pending: false, error: null, nodes, selectedCandidateId: null,
      },
    })
    expect(wrapper.find('details.state-traces').exists()).toBe(false)
  })

  /**
   * An outcome that rests near zero and then saturates is a five-digit
   * percentage of its resting level, which is the number this view existed to
   * stop reporting. Past a factor of ten the multiple is how anyone would say it.
   */
  it('reads a large ratio as a multiple rather than a five-digit percentage', () => {
    const surge: ScenarioAnalysis = {
      ...analysis,
      candidates: [{
        ...analysis.candidates[0]!,
        objectives: [{
          ...analysis.candidates[0]!.objectives[0]!,
          direction: 'minimize',
          baseline: { ...estimate, mean: 0.0199 },
          final_state: { ...estimate, mean: 95.6 },
          improvement: { ...estimate, mean: -95.58 },
        }],
      }],
    }
    const wrapper = mount(OptimizeAnalysisPanel, {
      props: {
        scenarios: [scenario], selectedScenarioId: 'A', analysis: surge,
        pending: false, error: null, nodes, selectedCandidateId: null,
      },
    })

    expect(wrapper.get('.relative-impact').text()).toBe('4.8kx worse')
    expect(wrapper.get('.relative-impact').attributes('data-impact')).toBe('negative')
  })

  it('labels direction-oriented losses as regressions', () => {
    const regressionAnalysis: ScenarioAnalysis = {
      ...analysis,
      candidates: [{
        ...analysis.candidates[0]!,
        objectives: [{
          ...analysis.candidates[0]!.objectives[0]!,
          final_state: { ...estimate, mean: 0.4 },
          improvement: { ...estimate, mean: -0.1 },
        }],
      }],
    }
    const wrapper = mount(OptimizeAnalysisPanel, {
      props: {
        scenarios: [scenario], selectedScenarioId: 'A', analysis: regressionAnalysis,
        pending: false, error: null, nodes, selectedCandidateId: null,
      },
    })

    expect(wrapper.get('.relative-impact').text()).toBe('20.0% worse')
    expect(wrapper.get('.relative-impact').attributes('data-impact')).toBe('negative')
  })

  it('separates an effect the horizon cut short from one that never arrives', () => {
    const slow: ScenarioAnalysis = {
      ...analysis,
      planning_horizon: 4,
      candidates: [{
        ...analysis.candidates[0]!,
        objectives: [{ ...analysis.candidates[0]!.objectives[0]!, periods_to_effect: 9 }],
      }],
    }
    const wrapper = mount(OptimizeAnalysisPanel, {
      props: {
        scenarios: [scenario], selectedScenarioId: 'A', analysis: slow,
        pending: false, error: null, nodes, selectedCandidateId: null,
      },
    })

    const warning = wrapper.get('.horizon-warning').text()
    expect(warning).toContain('Reliability')
    expect(warning).toContain('at least 9 periods')
    expect(warning).toContain('the horizon ended first, not that the intervention failed')
  })

  it('warns about every loop it cannot rule out, including one it cannot weigh', () => {
    const unstable: ScenarioAnalysis = {
      ...analysis,
      feedback_loops: [
        { states: ['A', 'B'], gain: 1.4, instability: 0.82, weights: [] },
        { states: ['A', 'C'], gain: 0.5, instability: 0, weights: [] },
        { states: ['B', 'C'], gain: null, instability: null, weights: [] },
      ],
    }
    const wrapper = mount(OptimizeAnalysisPanel, {
      props: {
        scenarios: [scenario], selectedScenarioId: 'A', analysis: unstable,
        pending: false, error: null, nodes, selectedCandidateId: null,
      },
    })

    const warning = wrapper.get('.stability-warning').text()
    expect(warning).toContain('2 feedback loops not shown to settle')
    expect(warning).toContain('gain 1.40')
    expect(warning).toContain('gain unknown')
    expect(warning).not.toContain('0.50')
  })

  /**
   * The case a point estimate hides: the mean contracts, so nothing above would
   * flag it, yet the sampled responses multiply past one in a fifth of draws.
   */
  it('warns about a loop whose mean settles but whose draws often do not', () => {
    const unstable: ScenarioAnalysis = {
      ...analysis,
      feedback_loops: [{ states: ['A', 'B'], gain: 0.81, instability: 0.21, weights: [] }],
    }
    const wrapper = mount(OptimizeAnalysisPanel, {
      props: {
        scenarios: [scenario], selectedScenarioId: 'A', analysis: unstable,
        pending: false, error: null, nodes, selectedCandidateId: null,
      },
    })

    const warning = wrapper.get('.stability-warning').text()
    expect(warning).toContain('1 feedback loop not shown to settle')
    expect(warning).toContain('gain 0.81 · runs away in 21% of draws')
  })
})
