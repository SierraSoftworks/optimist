<script setup lang="ts">
import { computed } from 'vue'
import { AlertTriangle, BarChart3, CheckCircle2, Clock3, GitBranch, Pencil, Plus, RefreshCw, Sparkles } from '@lucide/vue'
import type { GraphNode, Scenario, ScenarioAnalysis } from '../api/types'
import { impactTone, relativeImprovement } from '../domain/optimizationImpact'
import ScenarioPicker from './ScenarioPicker.vue'
import OptimizationTrajectory from './OptimizationTrajectory.vue'

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
  return invalid.non_finite_primitive + invalid.non_finite_result
}

function selectCandidate(candidate: ScenarioAnalysis['candidates'][number]) {
  emit('selectCandidate', candidate.intervention, [
    candidate.intervention,
    ...candidate.objectives.filter((objective) => objective.reachable).map((objective) => objective.outcome),
  ])
}

function objectiveImpact(objective: ScenarioAnalysis['candidates'][number]['objectives'][number]) {
  const baseline = objective.baseline.mean
  const finalState = objective.final_state.mean
  return impactTone(
    baseline === null || finalState === null ? null : finalState - baseline,
    objective.direction,
  )
}

function relativeImpact(objective: ScenarioAnalysis['candidates'][number]['objectives'][number]) {
  return relativeImprovement(objective.improvement.mean, objective.baseline.mean)
}

function relativeStandardError(objective: ScenarioAnalysis['candidates'][number]['objectives'][number]) {
  return relativeImprovement(objective.improvement.mean_standard_error, objective.baseline.mean)
}

function impactLabel(value: number | null) {
  if (value === null) return 'Unavailable'
  if (value === 0) return 'No change'
  return `${Math.abs(value * 100).toFixed(1)}% ${value > 0 ? 'improvement' : 'regression'}`
}

function percentagePoints(value: number | null) {
  return value === null ? 'Unavailable' : `${(value * 100).toFixed(1)} pp`
}
</script>

<template>
  <main class="analysis-panel optimize-panel" aria-label="Optimize analysis">
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
      <p class="analysis-boundary">Each candidate includes its prerequisite execution plan. Durations add, required success probabilities compound, and successful prerequisite effects are propagated before the candidate. Synergies are shown but remain qualitative until a magnitude is modelled.</p>
      <div v-if="analysis.candidates.length" class="candidate-list">
        <article v-for="candidate in analysis.candidates" :key="candidate.intervention" :class="{ selected: selectedCandidateId === candidate.intervention }">
          <button type="button" class="candidate-header" :aria-pressed="selectedCandidateId === candidate.intervention" @click="selectCandidate(candidate)">
            <span><strong>{{ title(candidate.intervention) }}</strong><small>{{ candidate.intervention }}</small></span>
            <span class="diagnostic-status" :data-status="candidate.diagnostics.status">{{ candidate.diagnostics.status.replaceAll('_', ' ') }}</span>
          </button>
          <dl class="candidate-diagnostics">
            <div><dt><Clock3 :size="11" /> Total duration</dt><dd>{{ number(candidate.execution_duration.mean) }} periods</dd></div>
            <div><dt><CheckCircle2 :size="11" /> Plan success</dt><dd>{{ candidate.execution_success.mean === null ? 'Unavailable' : `${(candidate.execution_success.mean * 100).toFixed(1)}%` }}</dd></div>
            <div><dt>Valid draws</dt><dd>{{ candidate.diagnostics.valid_samples }} / {{ candidate.diagnostics.attempted_samples }}</dd></div>
            <div><dt>Invalid draws</dt><dd>{{ invalidSamples(candidate) }}</dd></div>
            <div><dt>Clamped updates</dt><dd>{{ candidate.clamped_state_updates }}</dd></div>
            <div v-if="candidate.undefined_responses" class="negative"><dt>Undefined responses</dt><dd>{{ candidate.undefined_responses }}</dd></div>
            <div><dt>Seed</dt><dd>{{ candidate.diagnostics.seed }}</dd></div>
          </dl>
          <div v-if="candidate.prerequisites.length || candidate.blocking_requirements.length || candidate.synergies.length || candidate.conflicts.length" class="execution-context">
            <span v-if="candidate.prerequisites.length"><GitBranch :size="12" /> Requires first: {{ candidate.prerequisites.map(title).join(' → ') }}</span>
            <span v-if="candidate.blocking_requirements.length" class="negative"><AlertTriangle :size="12" /> {{ candidate.blocking_requirements.length }} factor requirement{{ candidate.blocking_requirements.length === 1 ? '' : 's' }}</span>
            <span v-if="candidate.synergies.length" class="positive"><Sparkles :size="12" /> Synergy: {{ candidate.synergies.map(title).join(', ') }}</span>
            <span v-if="candidate.conflicts.length" class="negative"><AlertTriangle :size="12" /> Conflicts: {{ candidate.conflicts.map(title).join(', ') }}</span>
          </div>
          <table class="projection-table">
            <caption class="sr-only">Objective projections for {{ title(candidate.intervention) }}</caption>
            <thead><tr><th scope="col">Objective</th><th scope="col">Impact vs baseline</th><th scope="col">MC SE</th></tr></thead>
            <tbody>
              <tr v-for="objective in candidate.objectives" :key="objective.outcome" :class="{ unreachable: !objective.reachable }">
                <th scope="row"><span>{{ title(objective.outcome) }}</span><small>{{ objective.reachable ? objective.direction : 'unreachable' }}</small></th>
                <td class="relative-impact" :data-impact="objectiveImpact(objective)">{{ impactLabel(relativeImpact(objective)) }}</td>
                <td>{{ percentagePoints(relativeStandardError(objective)) }}</td>
              </tr>
            </tbody>
          </table>
          <div class="trajectory-list">
            <OptimizationTrajectory
              v-for="objective in candidate.objectives.filter((objective) => objective.reachable)"
              :key="objective.outcome"
              :points="objective.trajectory"
              :label="title(objective.outcome)"
              :direction="objective.direction"
              :baseline="objective.baseline.mean"
            />
          </div>
        </article>
      </div>
      <div v-else class="analysis-empty">
        <BarChart3 :size="22" />
        <strong>No candidate projections</strong>
        <span>Add candidate interventions to this scenario before comparing outcomes.</span>
      </div>
    </template>
  </main>
</template>

<style scoped>
.scenario-selector-row { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: end; gap: 6px; margin-top: 15px; }
.scenario-edit-button { width: 32px; height: 32px; margin-bottom: 5px; border: 1px solid var(--line); background: white; }
.candidate-list { display: grid; gap: 9px; margin-top: 12px; }
.candidate-list article { overflow: hidden; border: 1px solid var(--line); border-radius: 6px; background: white; }
.candidate-list article.selected { border-color: #285c91; box-shadow: 0 0 0 1px #285c91; }
.candidate-header { width: 100%; display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 9px; border: 0; background: transparent; text-align: left; }
.candidate-header:hover, .candidate-header[aria-pressed='true'] { background: #edf3f9; }
.candidate-header > span:first-child { display: grid; gap: 2px; }
.candidate-header strong { font-size: 11px; }
.candidate-header small { color: var(--muted); font: 8px 'IBM Plex Mono', monospace; }
.diagnostic-status { padding: 3px 5px; border-radius: 4px; background: #edf0eb; color: var(--muted); font-size: 7px; font-weight: 700; text-transform: uppercase; }
.diagnostic-status[data-status='converged'] { background: var(--green-soft); color: var(--green); }
.diagnostic-status[data-status='maximum_samples_reached'], .diagnostic-status[data-status='insufficient_valid_samples'] { background: #fff2df; color: #8a5b00; }
.candidate-diagnostics { grid-template-columns: 1fr; gap: 0; padding: 0 9px 8px; }
.candidate-diagnostics div { grid-template-columns: 1fr auto; padding: 3px 0; border-bottom: 1px solid #eef0ec; font-size: 8px; }
.candidate-diagnostics dt { display: flex; align-items: center; gap: 4px; }
.execution-context { display: flex; flex-wrap: wrap; gap: 5px; padding: 0 9px 9px; }
.execution-context span { display: flex; align-items: center; gap: 4px; padding: 4px 6px; background: #edf1ed; color: #46554d; font-size: 8px; }
.execution-context .positive { background: #eaf5ed; color: #287044; }
.execution-context .negative { background: #fff0e8; color: #984335; }
.projection-table { width: 100%; border-collapse: collapse; border-top: 1px solid var(--line); font-size: 8px; }
.projection-table th, .projection-table td { padding: 6px 8px; text-align: right; }
.projection-table thead th { color: var(--muted); text-transform: uppercase; }
.projection-table th:first-child { text-align: left; }
.projection-table tbody th { display: grid; gap: 1px; }
.projection-table tbody th span { font-size: 9px; }
.projection-table tbody th small { color: var(--muted); font-size: 7px; font-weight: 400; text-transform: capitalize; }
.projection-table tr.unreachable { opacity: .55; }
.relative-impact { font-weight: 800; }
.relative-impact[data-impact='positive'] { color: #277445; }
.relative-impact[data-impact='negative'] { color: #a34335; }
.relative-impact[data-impact='neutral'] { color: var(--muted); }
.trajectory-list { display: grid; }
.optimize-panel { border: 0; padding: 24px clamp(18px, 4vw, 52px); background: #f4f6f1; }
.candidate-list { grid-template-columns: repeat(auto-fit, minmax(420px, 1fr)); align-items: start; }
</style>
