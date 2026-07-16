import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import type { GraphNode, ImpedimentAnalysis } from '../api/types'
import ImpedimentAnalysisPanel from './ImpedimentAnalysisPanel.vue'

const nodes: GraphNode[] = [
  {
    id: 'A', revision: 0, name: 'wide_reach', normalized_name: 'wide_reach', title: 'Wide reach',
    description: '', aliases: [], metadata: {},
    payload: { kind: 'factor', properties: { current: null, desired: null, controllable: true, evidence: [] } },
  },
  {
    id: 'B', revision: 0, name: 'documented', normalized_name: 'documented', title: 'Documented',
    description: '', aliases: [], metadata: {},
    payload: { kind: 'factor', properties: { current: null, desired: null, controllable: false, evidence: [] } },
  },
  ...['C', 'D'].map((id) => ({
    id, revision: 0, name: `outcome_${id}`, normalized_name: `outcome_${id}`, title: `Outcome ${id}`,
    description: '', aliases: [], metadata: {},
    payload: { kind: 'outcome' as const, properties: { direction: 'maximize' as const, current: null, desired: null, evidence: [] } },
  })),
]
const edge = (source: string, destination: string) => ({ source, kind: 'contributes' as const, destination })
const analysis: ImpedimentAnalysis = {
  revision: { project: 'A', graph_revision: 5, scenario: null, dependence_revision: null, formula_revision: 0 },
  topology_candidates: [
    {
      factor: 'A', controllable: true, reachable_outcomes: ['C', 'D'], nearest_outcome_distance: 1,
      path_edges: [edge('A', 'C'), edge('A', 'D')], direct_evidence: [], relationship_evidence: [],
      unsupported_path_edges: [edge('A', 'C'), edge('A', 'D')],
    },
    {
      factor: 'B', controllable: false, reachable_outcomes: ['C'], nearest_outcome_distance: 1,
      path_edges: [edge('B', 'C')],
      direct_evidence: [{ id: 0, revision: 0, summary: 'Observed', source: null }],
      relationship_evidence: [{ edge: edge('B', 'C'), references: ['ADR-1'] }],
      unsupported_path_edges: [],
    },
  ],
  evidence_priority: ['B', 'A'],
}

describe('ImpedimentAnalysisPanel', () => {
  it('keeps topology and evidence order separate while preserving candidate facts', async () => {
    const wrapper = mount(ImpedimentAnalysisPanel, {
      props: { analysis, pending: false, error: null, nodes, selectedFactorId: null },
    })
    expect(wrapper.findAll('.impediment-title strong').map((item) => item.text())).toEqual(['Wide reach', 'Documented'])
    expect(wrapper.text()).toContain('2 path edges lack typed evidence')
    await wrapper.get('button[aria-pressed="false"]').trigger('click')
    expect(wrapper.findAll('.impediment-title strong').map((item) => item.text())).toEqual(['Documented', 'Wide reach'])
    expect(wrapper.text()).toContain('Neither is a causal confidence score')
  })

  it('emits exact candidate nodes and path edges for graph highlighting', async () => {
    const wrapper = mount(ImpedimentAnalysisPanel, {
      props: { analysis, pending: false, error: null, nodes, selectedFactorId: null },
    })
    await wrapper.findAll('.impediment-list > li > button')[0]!.trigger('click')
    expect(wrapper.emitted('select')![0]).toEqual(['A', ['A', 'C', 'D'], [edge('A', 'C'), edge('A', 'D')]])
  })
})
