<script setup lang="ts">
import { computed } from 'vue'
import { AlertTriangle, BarChart3, CheckCircle2, ChevronRight, Clock3, GitBranch, Pencil, Plus, RefreshCw, Sparkles } from '@lucide/vue'
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

/**
 * Reports objectives the horizon ends before their effect can arrive.
 *
 * Such an objective reports the same flat zero as an unreachable one, so without
 * this the reader cannot tell "no effect" from "not yet".
 */
function truncated(candidate: ScenarioAnalysis['candidates'][number]) {
  const horizon = props.analysis?.planning_horizon ?? 0
  return candidate.objectives.filter(
    (objective) => objective.periods_to_effect !== null && objective.periods_to_effect > horizon,
  )
}

/**
 * Loops that cannot be shown to settle.
 *
 * An unknown gain is not a safe one: a loop closed through a node equation admits
 * no elasticity to multiply, yet it can run away just as far. Only a known,
 * contracting gain is excluded.
 */
const unsettledLoops = computed(
  () => props.analysis?.feedback_loops.filter((loop) => loop.gain === null || Math.abs(loop.gain) >= 1) ?? [],
)

function gainLabel(gain: number | null) {
  return gain === null ? 'gain unknown' : `gain ${gain.toFixed(2)}`
}

/**
 * Counters worth reading only when they are not zero.
 *
 * A healthy run reports no invalid draws, no clamping, and no undefined
 * responses, so printing all three every time trained the eye to skip the block
 * that matters when one of them fires.
 */
function concerns(candidate: ScenarioAnalysis['candidates'][number]) {
  return [
    { label: 'Invalid draws', value: invalidSamples(candidate) },
    { label: 'Clamped updates', value: candidate.clamped_state_updates },
    { label: 'Undefined responses', value: candidate.undefined_responses },
  ].filter((entry) => entry.value > 0)
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
      <p class="scenario-meta">
        <span><strong>{{ analysis.candidates.length }}</strong> candidates</span>
        <span><strong>{{ selectedScenario.objectives.length }}</strong> objectives</span>
        <span><strong>{{ analysis.planning_horizon }}</strong> periods</span>
      </p>
      <details class="detail-disclosure projection-scope">
        <summary><ChevronRight :size="13" /> What this projection assumes</summary>
        <p class="analysis-boundary">Each candidate includes its prerequisite execution plan. Durations add, required success probabilities compound, and successful prerequisite effects are propagated before the candidate. Synergies are shown but remain qualitative until a magnitude is modelled.</p>
      </details>
      <div v-if="unsettledLoops.length" class="stability-warning">
        <AlertTriangle :size="15" />
        <div>
          <strong>{{ unsettledLoops.length }} feedback loop{{ unsettledLoops.length === 1 ? '' : 's' }} not shown to settle</strong>
          <ul>
            <li v-for="(loop, index) in unsettledLoops" :key="index">
              {{ loop.states.map(title).join(' → ') }} → {{ title(loop.states[0]!) }}
              <code>{{ gainLabel(loop.gain) }}</code>
            </li>
          </ul>
          <span>A deviation entering these loops is not shown to decay, so it grows each period until the destination's declared support clamps it and the projection reports that bound more than the intervention. An unknown gain means a node equation sits on the loop, which admits no elasticity to multiply rather than being safe.</span>
        </div>
      </div>
      <div v-if="analysis.candidates.length" class="candidate-list">
        <article v-for="candidate in analysis.candidates" :key="candidate.intervention" :class="{ selected: selectedCandidateId === candidate.intervention }">
          <button type="button" class="candidate-header" :aria-pressed="selectedCandidateId === candidate.intervention" @click="selectCandidate(candidate)">
            <span><strong>{{ title(candidate.intervention) }}</strong><small>{{ candidate.intervention }}</small></span>
            <span
              v-if="candidate.diagnostics.status !== 'converged'"
              class="diagnostic-status"
              :data-status="candidate.diagnostics.status"
            >{{ candidate.diagnostics.status.replaceAll('_', ' ') }}</span>
          </button>
          <dl class="candidate-diagnostics">
            <div><dt><Clock3 :size="13" /> Total duration</dt><dd>{{ number(candidate.execution_duration.mean) }} periods</dd></div>
            <div><dt><CheckCircle2 :size="13" /> Plan success</dt><dd>{{ candidate.execution_success.mean === null ? 'Unavailable' : `${(candidate.execution_success.mean * 100).toFixed(1)}%` }}</dd></div>
            <div v-for="concern in concerns(candidate)" :key="concern.label" class="negative">
              <dt><AlertTriangle :size="13" /> {{ concern.label }}</dt><dd>{{ concern.value }}</dd>
            </div>
          </dl>
          <details class="detail-disclosure">
            <summary><ChevronRight :size="13" /> Run detail</summary>
            <dl class="fact-list">
              <div><dt>Valid draws</dt><dd>{{ candidate.diagnostics.valid_samples }} / {{ candidate.diagnostics.attempted_samples }}</dd></div>
              <div><dt>Invalid draws</dt><dd>{{ invalidSamples(candidate) }}</dd></div>
              <div><dt>Clamped updates</dt><dd>{{ candidate.clamped_state_updates }}</dd></div>
              <div><dt>Undefined responses</dt><dd>{{ candidate.undefined_responses }}</dd></div>
              <div><dt>Convergence</dt><dd>{{ candidate.diagnostics.status.replaceAll('_', ' ') }}</dd></div>
              <div><dt>Seed</dt><dd>{{ candidate.diagnostics.seed }}</dd></div>
            </dl>
          </details>
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
          <p v-if="truncated(candidate).length" class="horizon-warning">
            <Clock3 :size="13" />
            <span>
              {{ truncated(candidate).map((objective) => title(objective.outcome)).join(', ') }}
              {{ truncated(candidate).length === 1 ? 'needs' : 'need' }} at least
              {{ Math.max(...truncated(candidate).map((objective) => objective.periods_to_effect!)) }}
              periods to respond, beyond this scenario's {{ analysis.planning_horizon }}. Their flat
              result means the horizon ended first, not that the intervention failed.
            </span>
          </p>
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
.scenario-selector-row { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: end; gap: var(--space-2); margin-top: var(--space-4); }
.scenario-meta { display: flex; flex-wrap: wrap; gap: var(--space-4); margin: var(--space-3) 0 0; color: var(--muted); font-size: var(--text-sm); }
.scenario-meta strong { color: var(--ink); font-family: var(--mono); font-size: var(--text-md); }
.projection-scope { margin-top: var(--space-3); }
.projection-scope .analysis-boundary { margin-top: var(--space-2); }
.scenario-edit-button { width: 36px; height: 36px; margin-bottom: 4px; border: 1px solid var(--line); background: white; }
.candidate-list { display: grid; gap: var(--space-3); margin-top: var(--space-4); }
.candidate-list article { overflow: hidden; border: 1px solid var(--line); border-radius: var(--radius-lg); background: white; }
.candidate-list article.selected { border-color: #285c91; box-shadow: 0 0 0 1px #285c91; }
.candidate-header { width: 100%; display: flex; align-items: center; justify-content: space-between; gap: var(--space-2); padding: var(--space-3) var(--space-4); border: 0; background: transparent; text-align: left; }
.candidate-header:hover, .candidate-header[aria-pressed='true'] { background: #edf3f9; }
.candidate-header > span:first-child { display: grid; gap: 2px; }
.candidate-header strong { font-size: var(--text-lg); }
.candidate-header small { color: var(--muted); font: var(--text-2xs) var(--mono); }
.diagnostic-status { flex: none; padding: 4px 7px; border-radius: var(--radius-sm); background: #edf0eb; color: var(--muted); font-size: var(--text-2xs); font-weight: 700; text-transform: uppercase; letter-spacing: .04em; }
.diagnostic-status[data-status='maximum_samples_reached'], .diagnostic-status[data-status='insufficient_valid_samples'] { background: #fff2df; color: #8a5b00; }
.candidate-diagnostics { grid-template-columns: 1fr; gap: 0; padding: 0 var(--space-4) var(--space-2); }
.candidate-diagnostics div { grid-template-columns: 1fr auto; padding: 6px 0; border-bottom: 1px solid #eef0ec; font-size: var(--text-md); }
.candidate-diagnostics dt { display: flex; align-items: center; gap: 6px; }
.candidate-diagnostics dd { font-family: var(--mono); font-size: var(--text-sm); }
.candidate-diagnostics .negative dt, .candidate-diagnostics .negative dd { color: #984335; }
.detail-disclosure { margin: 0 var(--space-4) var(--space-3); }
.execution-context { display: flex; flex-wrap: wrap; gap: 6px; padding: 0 var(--space-4) var(--space-3); }
.execution-context span { display: flex; align-items: center; gap: 5px; padding: 5px 8px; border-radius: var(--radius-sm); background: #edf1ed; color: #46554d; font-size: var(--text-xs); }
.execution-context .positive { background: #eaf5ed; color: #287044; }
.execution-context .negative { background: #fff0e8; color: #984335; }
.projection-table { width: 100%; border-collapse: collapse; border-top: 1px solid var(--line); font-size: var(--text-sm); }
.projection-table th, .projection-table td { padding: var(--space-2) var(--space-4); text-align: right; }
.projection-table thead th { color: var(--muted); font-size: var(--text-2xs); text-transform: uppercase; letter-spacing: .05em; }
.projection-table th:first-child { text-align: left; }
.projection-table tbody th { display: grid; gap: 2px; }
.projection-table tbody th span { font-size: var(--text-md); }
.projection-table tbody th small { color: var(--muted); font-size: var(--text-xs); font-weight: 400; text-transform: capitalize; }
.projection-table tr.unreachable { opacity: .55; }
.horizon-warning { display: flex; gap: 7px; align-items: flex-start; margin: 0; padding: var(--space-2) var(--space-4) var(--space-3); color: #8a6206; font-size: var(--text-sm); line-height: 1.5; }
.horizon-warning svg { flex: none; margin-top: 2px; }
.stability-warning { display: flex; gap: var(--space-3); align-items: flex-start; margin-top: var(--space-3); padding: var(--space-3) var(--space-4); border: 1px solid var(--caution-line); border-radius: var(--radius-md); background: var(--caution-surface); color: var(--caution); }
.stability-warning svg { flex: none; margin-top: 2px; }
.stability-warning strong { display: block; font-size: var(--text-md); }
.stability-warning ul { margin: 6px 0; padding-left: 18px; font-size: var(--text-sm); line-height: 1.6; }
.stability-warning code { padding: 1px 4px; border-radius: 3px; background: #f3e6c4; font: var(--text-xs) var(--mono); }
.stability-warning span { display: block; font-size: var(--text-sm); line-height: 1.55; opacity: .85; }
.relative-impact { font-weight: 800; font-size: var(--text-md); }
.relative-impact[data-impact='positive'] { color: #277445; }
.relative-impact[data-impact='negative'] { color: #a34335; }
.relative-impact[data-impact='neutral'] { color: var(--muted); }
.trajectory-list { display: grid; }
.optimize-panel { border: 0; background: #f4f6f1; }
/*
 * Cards get wider before they get more numerous, so a large display shows two
 * readable comparisons rather than four cramped ones.
 */
.candidate-list { grid-template-columns: repeat(auto-fit, minmax(min(100%, 520px), 1fr)); align-items: start; }
</style>
