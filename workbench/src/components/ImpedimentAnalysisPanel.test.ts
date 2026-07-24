import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import type { GraphNode, ImpedimentAnalysis } from '../api/types'
import ImpedimentAnalysisPanel from './ImpedimentAnalysisPanel.vue'

const intervention = (id: string, title: string): GraphNode => ({
  id, revision: 0, name: title.toLowerCase(), normalized_name: title.toLowerCase(), title,
  description: '', aliases: [], metadata: {},
  payload: { kind: 'intervention', properties: { costs: [], duration: null, probability_of_success: null, acceptance_criteria: [] } },
})
const nodes = [intervention('A', 'Platform'), intervention('B', 'Automation'), intervention('C', 'Training')]
const analysis: ImpedimentAnalysis = {
  revision: { project: 'A', graph_revision: 5, scenario: null, dependence_revision: null },
  candidates: [{
    intervention: 'B',
    execution_steps: [
      { intervention: 'A', duration: { type: 'point', value: 2 }, probability_of_success: { type: 'point', value: 0.8 } },
      { intervention: 'B', duration: { type: 'point', value: 3 }, probability_of_success: { type: 'point', value: 0.5 } },
    ],
    blocking_requirements: [],
    synergies: ['C'], conflicts: [], expected_duration: 5, expected_success_probability: 0.4,
  }],
}

describe('ImpedimentAnalysisPanel', () => {
  it('shows prerequisite order, combined execution metrics, and synergies', () => {
    const wrapper = mount(ImpedimentAnalysisPanel, {
      props: { analysis, pending: false, error: null, nodes },
    })
    expect(wrapper.text()).toContain('Automation')
    expect(wrapper.text()).toContain('5 periods')
    expect(wrapper.text()).toContain('40.0%')
    expect(wrapper.findAll('.execution-plan li').map((item) => item.text())).toEqual([
      expect.stringContaining('Platform'),
      expect.stringContaining('Automation'),
    ])
    expect(wrapper.text()).toContain('Synergy: Training')
  })
})
