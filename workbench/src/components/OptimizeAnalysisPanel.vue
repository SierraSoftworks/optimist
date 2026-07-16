<script setup lang="ts">
import { computed } from 'vue'
import { AlertTriangle, BarChart3, Pencil, Plus, RefreshCw } from '@lucide/vue'
import type { GraphNode, Scenario, ScenarioAnalysis } from '../api/types'
import ScenarioPicker from './ScenarioPicker.vue'

const props = defineProps<{
  scenarios: Scenario[]
  selectedScenarioId: string | null
  analysis: ScenarioAnalysis | undefined
  pending: boolean
  error: Error | null
  nodes: GraphNode[]
  selectedCandidateId: string | null
}>()
const emit = defineEmits<{
  selectScenario: [id: string]
  selectCandidate: [id: string, highlightedNodes: string[]]
  create: []
  edit: []
  retry: []
}>()
const selectedScenario = computed(() =>
  props.scenarios.find((scenario) => scenario.id === props.selectedScenarioId) ?? null,
)
const nodeTitles = computed(() => new Map(props.nodes.map((node) => [node.id, node.title])))

function title(id: string) {
  return nodeTitles.value.get(id) ?? id
}

function number(value: number | null, digits = 3) {
  return value === null ? 'Unavailable' : Number(value.toFixed(digits)).toString()
}

function invalidSamples(candidate: ScenarioAnalysis['candidates'][number]) {
  const invalid = candidate.diagnostics.invalid_samples
  return invalid.zero_denominator + invalid.non_finite_primitive + invalid.non_finite_result
}

function selectCandidate(candidate: ScenarioAnalysis['candidates'][number]) {
  emit('selectCandidate', candidate.intervention, [
    candidate.intervention,
    ...candidate.objectives.filter((objective) => objective.reachable).map((objective) => objective.outcome),
  ])
}
</script>

<template>
  <aside class="analysis-panel optimize-panel" aria-label="Optimize analysis">
    <header class="analysis-panel-header">
      <div><span class="eyebrow">Finite-horizon projection</span><h2>Candidate comparison</h2></div>
      <button type="button" class="icon-button" title="Create scenario" aria-label="Create scenario" @click="emit('create')"><Plus :size="16" /></button>
    </header>

    <div v-if="scenarios.length" class="scenario-selector-row">
      <ScenarioPicker :scenarios="scenarios" :selected-scenario-id="selectedScenarioId" @select="emit('selectScenario', $event)" @create="emit('create')" />
      <button type="button" class="icon-button scenario-edit-button" title="Edit selected scenario" aria-label="Edit selected scenario" @click="emit('edit')"><Pencil :size="15" /></button>
    </div>

    <div v-if="!scenarios.length && !pending" class="analysis-empty">
      <BarChart3 :size="22" />
      <strong>No scenarios yet</strong>
      <span>Create a scenario to compare intervention candidates against explicit outcome objectives.</span>
      <button type="button" class="primary-button" @click="emit('create')"><Plus :size="15" /> Create scenario</button>
    </div>
    <div v-else-if="pending" class="analysis-state"><RefreshCw class="spin" :size="20" /><span>Projecting scenario candidates</span></div>
    <div v-else-if="error" class="analysis-state analysis-error">
      <AlertTriangle :size="20" />
      <strong>Projection unavailable</strong>
      <span>{{ error.message }}</span>
      <button type="button" class="secondary-button" @click="emit('retry')">Retry</button>
    </div>
    <template v-else-if="analysis && selectedScenario">
      <div class="analysis-summary optimize-summary">
        <div><strong>{{ analysis.candidates.length }}</strong><span>candidates</span></div>
        <div><strong>{{ selectedScenario.objectives.length }}</strong><span>objectives</span></div>
        <div><strong>{{ analysis.planning_horizon }}</strong><span>periods</span></div>
      </div>
      <p class="analysis-boundary">Candidates are projected independently. No budget, bundle, conflict, synergy, or scalar ranking is applied.</p>
      <div v-if="analysis.candidates.length" class="candidate-list">
        <article v-for="candidate in analysis.candidates" :key="candidate.intervention" :class="{ selected: selectedCandidateId === candidate.intervention }">
          <button type="button" class="candidate-header" :aria-pressed="selectedCandidateId === candidate.intervention" @click="selectCandidate(candidate)">
            <span><strong>{{ title(candidate.intervention) }}</strong><small>{{ candidate.intervention }}</small></span>
            <span class="diagnostic-status" :data-status="candidate.diagnostics.status">{{ candidate.diagnostics.status.replaceAll('_', ' ') }}</span>
          </button>
          <dl class="candidate-diagnostics">
            <div><dt>Valid draws</dt><dd>{{ candidate.diagnostics.valid_samples }} / {{ candidate.diagnostics.attempted_samples }}</dd></div>
            <div><dt>Invalid draws</dt><dd>{{ invalidSamples(candidate) }}</dd></div>
            <div><dt>Clamped updates</dt><dd>{{ candidate.clamped_state_updates }}</dd></div>
            <div><dt>Seed</dt><dd>{{ candidate.diagnostics.seed }}</dd></div>
          </dl>
          <table class="projection-table">
            <caption class="sr-only">Objective projections for {{ title(candidate.intervention) }}</caption>
            <thead><tr><th scope="col">Objective</th><th scope="col">Improvement</th><th scope="col">MC SE</th></tr></thead>
            <tbody>
              <tr v-for="objective in candidate.objectives" :key="objective.outcome" :class="{ unreachable: !objective.reachable }">
                <th scope="row"><span>{{ title(objective.outcome) }}</span><small>{{ objective.reachable ? objective.direction : 'unreachable' }}</small></th>
                <td>{{ number(objective.improvement.mean) }}</td>
                <td>{{ number(objective.improvement.mean_standard_error) }}</td>
              </tr>
            </tbody>
          </table>
        </article>
      </div>
      <div v-else class="analysis-empty">
        <BarChart3 :size="22" />
        <strong>No candidate projections</strong>
        <span>Add candidate interventions to this scenario before comparing outcomes.</span>
      </div>
    </template>
  </aside>
</template>
