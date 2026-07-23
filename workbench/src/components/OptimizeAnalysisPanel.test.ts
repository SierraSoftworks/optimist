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
const trajectory = [
  { period: 0, state: { ...estimate, mean: 0.5 }, improvement: { ...estimate, mean: 0 } },
  { period: 12, state: { ...estimate, mean: 0.62 }, improvement: estimate },
]
const analysis: ScenarioAnalysis = {
  revision: { project: 'A', graph_revision: 5, scenario: ['A', 0], dependence_revision: null },
  planning_horizon: 12,
  candidates: [{
    intervention: 'B',
    objectives: [{
      outcome: 'A', direction: 'maximize', importance: 1, reachable: true,
      baseline: estimate, final_state: estimate, improvement: estimate, trajectory,
    }],
    improvement_covariance: [[0.02]],
    clamped_state_updates: 3,
    diagnostics: {
      seed: 42, attempted_samples: 120, valid_samples: 118,
      invalid_samples: { non_finite_primitive: 2, non_finite_result: 0 },
      criterion: config, status: 'converged',
    },
  }],
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
    expect(wrapper.text()).toContain('0.12')
    expect(wrapper.text()).toContain('0.004')
    expect(wrapper.text()).toContain('118 / 120')
    expect(wrapper.text()).toContain('No budget, bundle, conflict, synergy, or scalar ranking')
    expect(wrapper.get('figure[aria-label="Reliability improvement over time"]').text()).toContain('12 periods')
    await wrapper.get('.candidate-header').trigger('click')
    expect(wrapper.emitted('selectCandidate')![0]).toEqual(['B', ['B', 'A']])
  })
})
