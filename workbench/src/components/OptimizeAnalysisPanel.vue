<script setup lang="ts">
import { computed } from 'vue'
import { AlertTriangle, ArrowRight, BarChart3, CheckCircle2, ChevronRight, Clock3, GitBranch, Pencil, Plus, RefreshCw, Sparkles } from '@lucide/vue'
import type { FeedbackLoop, GraphNode, Scenario, ScenarioAnalysis } from '../api/types'
import { impactTone } from '../domain/optimizationImpact'
import { formatSiNumber } from '../domain/humanNumber'
import ScenarioPicker from './ScenarioPicker.vue'
import OutcomeTrajectory from './OutcomeTrajectory.vue'
import StateTrace from './StateTrace.vue'
import { referenceCandidate, referenceStates } from '../domain/optimizationReference'

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
/**
 * Candidate whose detail the pane shows.
 *
 * Falls back to the first so the pane is never empty while candidates exist:
 * the selection is reconciled asynchronously once the projection arrives, and
 * showing nothing in two thirds of the view in the meantime helps no one.
 */
const selectedCandidate = computed(() => {
  const candidates = props.analysis?.candidates ?? []
  return candidates.find((candidate) => candidate.intervention === props.selectedCandidateId)
    ?? candidates[0]
    ?? null
})
const nodeTitles = computed(() => new Map(props.nodes.map((node) => [node.id, node.title])))

/**
 * Native unit of each state-bearing node, so an outcome can be plotted in it.
 *
 * An outcome keeps its quantity in `native_state`; a metric keeps the same shape
 * inside its payload. Reading both means the axis carries a unit whichever kind
 * a scenario names as its objective.
 */
const nodeUnits = computed(() => new Map(props.nodes.map((node) => {
  const quantity = node.native_state?.quantity
    ?? (node.payload.kind === 'metric' ? node.payload.properties.quantity : undefined)
  return [node.id, quantity?.unit ?? null]
})))

function unit(id: string) {
  return nodeUnits.value.get(id) ?? null
}

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

/**
 * The run a candidate should be read against, and its settled outcome value.
 *
 * Measuring against the resting baseline produced the six-digit percentages this
 * view used to show: an outcome resting near zero and settling in the hundreds is
 * a 380,000% regression against rest, and every candidate under the same load
 * surge reports the same uninformative number. Against the run the candidate
 * actually deviates from, the figure is the decision.
 */
function withoutValue(
  candidate: ScenarioAnalysis['candidates'][number],
  objective: ScenarioAnalysis['candidates'][number]['objectives'][number],
) {
  const reference = referenceCandidate(candidate, props.analysis?.candidates ?? [])
  const projected = reference?.objectives.find((entry) => entry.outcome === objective.outcome)
  return projected?.final_state.mean ?? objective.baseline.mean
}

function change(
  candidate: ScenarioAnalysis['candidates'][number],
  objective: ScenarioAnalysis['candidates'][number]['objectives'][number],
) {
  const without = withoutValue(candidate, objective)
  const settled = objective.final_state.mean
  if (without === null || settled === null) return null
  return settled - without
}

function changeTone(
  candidate: ScenarioAnalysis['candidates'][number],
  objective: ScenarioAnalysis['candidates'][number]['objectives'][number],
) {
  return impactTone(change(candidate, objective), objective.direction)
}

function changeLabel(
  candidate: ScenarioAnalysis['candidates'][number],
  objective: ScenarioAnalysis['candidates'][number]['objectives'][number],
) {
  const shift = change(candidate, objective)
  if (shift === null) return 'Unavailable'
  if (shift === 0) return 'No change'
  const without = withoutValue(candidate, objective)
  const settled = objective.final_state.mean
  const improves = objective.direction === 'maximize' ? shift > 0 : shift < 0
  const wording = improves ? 'better' : 'worse'
  if (without === null || without === 0 || settled === null) {
    return `${quantity(Math.abs(shift), objective.outcome)} ${wording}`
  }
  // A candidate with no prerequisites is read against rest, and an outcome that
  // rests near zero then saturates is a five-digit percentage of it. Past a
  // factor of ten the multiple is how anyone would actually say it.
  const ratio = Math.abs(settled / without)
  if (ratio >= 10 || (ratio > 0 && ratio <= 0.1)) {
    return `${formatSiNumber(ratio >= 10 ? ratio : 1 / ratio)}x ${wording}`
  }
  return `${(Math.abs(shift / without) * 100).toFixed(1)}% ${wording}`
}

function quantity(value: number | null, outcome: string) {
  if (value === null) return 'Unavailable'
  const rounded = Number(value.toPrecision(3)).toString()
  const suffix = unit(outcome)
  return suffix ? `${rounded} ${suffix}` : rounded
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
 * no elasticity to multiply, yet it can run away just as far. Nor is a mean below
 * one, when the sampled product crosses it often enough to matter.
 */
const unsettledLoops = computed(() => props.analysis?.feedback_loops.filter(
  (loop) => loop.gain === null || Math.abs(loop.gain) >= 1 || (loop.instability ?? 0) >= 0.05,
) ?? [])

function gainLabel(loop: FeedbackLoop) {
  const gain = loop.gain === null ? 'gain unknown' : `gain ${loop.gain.toFixed(2)}`
  return loop.instability === null || loop.instability === 0
    ? gain
    : `${gain} · runs away in ${(loop.instability * 100).toFixed(0)}% of draws`
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
              <code>{{ gainLabel(loop) }}</code>
            </li>
          </ul>
          <span>A deviation entering these loops is not shown to decay, so it grows each period until the destination's declared support clamps it and the projection reports that bound more than the intervention. A mean gain below one is not on its own a guarantee: what matters is how often the sampled responses multiply past it. An unknown gain means a node equation sits on the loop, which admits no elasticity to multiply rather than being safe.</span>
        </div>
      </div>
      <div v-if="analysis.candidates.length" class="optimize-workspace">
        <nav class="candidate-rail" aria-label="Intervention candidates">
          <button
            v-for="candidate in analysis.candidates"
            :key="candidate.intervention"
            type="button"
            class="candidate-summary"
            :class="{ selected: selectedCandidateId === candidate.intervention }"
            :aria-pressed="selectedCandidateId === candidate.intervention"
            @click="selectCandidate(candidate)"
          >
            <span class="candidate-name">
              <strong>{{ title(candidate.intervention) }}</strong>
              <small>{{ candidate.intervention }}</small>
            </span>
            <span
              v-if="candidate.diagnostics.status !== 'converged'"
              class="diagnostic-status"
              :data-status="candidate.diagnostics.status"
            >{{ candidate.diagnostics.status.replaceAll('_', ' ') }}</span>
            <span
              v-for="objective in candidate.objectives"
              :key="objective.outcome"
              class="summary-objective"
            >
              <span class="summary-outcome">{{ title(objective.outcome) }}</span>
              <span class="summary-shift">
                <span>{{ quantity(withoutValue(candidate, objective), objective.outcome) }}</span>
                <ArrowRight :size="11" />
                <span>{{ quantity(objective.final_state.mean, objective.outcome) }}</span>
              </span>
              <span class="summary-change" :data-impact="changeTone(candidate, objective)">
                {{ changeLabel(candidate, objective) }}
              </span>
            </span>
          </button>
        </nav>
        <section v-if="selectedCandidate" class="candidate-detail" :aria-label="`${title(selectedCandidate.intervention)} projection detail`">
          <article :key="selectedCandidate.intervention">
            <header class="detail-header">
              <div>
                <span class="eyebrow">Candidate</span>
                <h3>{{ title(selectedCandidate.intervention) }}</h3>
              </div>
              <span
                v-if="selectedCandidate.diagnostics.status !== 'converged'"
                class="diagnostic-status"
                :data-status="selectedCandidate.diagnostics.status"
              >{{ selectedCandidate.diagnostics.status.replaceAll('_', ' ') }}</span>
            </header>
            <dl class="candidate-diagnostics">
              <div><dt><Clock3 :size="13" /> Total duration</dt><dd>{{ number(selectedCandidate.execution_duration.mean) }} periods</dd></div>
              <div><dt><CheckCircle2 :size="13" /> Plan success</dt><dd>{{ selectedCandidate.execution_success.mean === null ? 'Unavailable' : `${(selectedCandidate.execution_success.mean * 100).toFixed(1)}%` }}</dd></div>
              <div v-for="concern in concerns(selectedCandidate)" :key="concern.label" class="negative">
                <dt><AlertTriangle :size="13" /> {{ concern.label }}</dt><dd>{{ concern.value }}</dd>
              </div>
            </dl>
            <div v-if="selectedCandidate.prerequisites.length || selectedCandidate.blocking_requirements.length || selectedCandidate.synergies.length || selectedCandidate.conflicts.length" class="execution-context">
              <span v-if="selectedCandidate.prerequisites.length"><GitBranch :size="12" /> Requires first: {{ selectedCandidate.prerequisites.map(title).join(' → ') }}</span>
              <span v-if="selectedCandidate.blocking_requirements.length" class="negative"><AlertTriangle :size="12" /> {{ selectedCandidate.blocking_requirements.length }} factor requirement{{ selectedCandidate.blocking_requirements.length === 1 ? '' : 's' }}</span>
              <span v-if="selectedCandidate.synergies.length" class="positive"><Sparkles :size="12" /> Synergy: {{ selectedCandidate.synergies.map(title).join(', ') }}</span>
              <span v-if="selectedCandidate.conflicts.length" class="negative"><AlertTriangle :size="12" /> Conflicts: {{ selectedCandidate.conflicts.map(title).join(', ') }}</span>
            </div>
            <table class="projection-table">
              <caption class="sr-only">Objective projections for {{ title(selectedCandidate.intervention) }}</caption>
              <thead><tr><th scope="col">Objective</th><th scope="col">Without</th><th scope="col">With</th><th scope="col">Change</th></tr></thead>
              <tbody>
                <tr v-for="objective in selectedCandidate.objectives" :key="objective.outcome" :class="{ unreachable: !objective.reachable }">
                  <th scope="row"><span>{{ title(objective.outcome) }}</span><small>{{ objective.reachable ? objective.direction : 'unreachable' }}</small></th>
                  <td class="settled">{{ quantity(withoutValue(selectedCandidate, objective), objective.outcome) }}</td>
                  <td class="settled">{{ quantity(objective.final_state.mean, objective.outcome) }}</td>
                  <td class="relative-impact" :data-impact="changeTone(selectedCandidate, objective)">{{ changeLabel(selectedCandidate, objective) }}</td>
                </tr>
              </tbody>
            </table>
            <p v-if="truncated(selectedCandidate).length" class="horizon-warning">
              <Clock3 :size="13" />
              <span>
                {{ truncated(selectedCandidate).map((objective) => title(objective.outcome)).join(', ') }}
                {{ truncated(selectedCandidate).length === 1 ? 'needs' : 'need' }} at least
                {{ Math.max(...truncated(selectedCandidate).map((objective) => objective.periods_to_effect!)) }}
                periods to respond, beyond this scenario's {{ analysis.planning_horizon }}. Their flat
                result means the horizon ended first, not that the intervention failed.
              </span>
            </p>
            <div class="trajectory-list">
              <OutcomeTrajectory
                v-for="objective in selectedCandidate.objectives.filter((objective) => objective.reachable)"
                :key="objective.outcome"
                :points="objective.trajectory"
                :reference="referenceStates(
                  selectedCandidate,
                  objective.outcome,
                  referenceCandidate(selectedCandidate, analysis.candidates),
                  objective.trajectory.length,
                )"
                :label="title(objective.outcome)"
                :unit="unit(objective.outcome)"
                :direction="objective.direction"
                :projected-reference="referenceCandidate(selectedCandidate, analysis.candidates) !== null"
              />
            </div>
            <details class="detail-disclosure">
              <summary><ChevronRight :size="13" /> Run detail</summary>
              <dl class="fact-list">
                <div><dt>Valid draws</dt><dd>{{ selectedCandidate.diagnostics.valid_samples }} / {{ selectedCandidate.diagnostics.attempted_samples }}</dd></div>
                <div><dt>Invalid draws</dt><dd>{{ invalidSamples(selectedCandidate) }}</dd></div>
                <div><dt>Clamped updates</dt><dd>{{ selectedCandidate.clamped_state_updates }}</dd></div>
                <div><dt>Undefined responses</dt><dd>{{ selectedCandidate.undefined_responses }}</dd></div>
                <div><dt>Convergence</dt><dd>{{ selectedCandidate.diagnostics.status.replaceAll('_', ' ') }}</dd></div>
                <div><dt>Seed</dt><dd>{{ selectedCandidate.diagnostics.seed }}</dd></div>
              </dl>
            </details>
            <details v-if="selectedCandidate.states?.length" class="detail-disclosure state-traces">
              <summary><ChevronRight :size="13" /> Model states under this plan ({{ selectedCandidate.states.length }})</summary>
              <p class="muted-note">
                Every propagated state, in the order the graph settles them. A path that never
                moves, or moves somewhere the unit cannot mean, is where a surprising projection
                usually starts.
              </p>
              <div class="state-grid">
                <StateTrace
                  v-for="path in selectedCandidate.states"
                  :key="path.state"
                  :points="path.points"
                  :label="title(path.state)"
                  :unit="unit(path.state)"
                />
              </div>
            </details>
          </article>
        </section>
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
.diagnostic-status { flex: none; padding: 4px 7px; border-radius: var(--radius-sm); background: #edf0eb; color: var(--muted); font-size: var(--text-2xs); font-weight: 700; text-transform: uppercase; letter-spacing: .04em; }
.diagnostic-status[data-status='maximum_samples_reached'], .diagnostic-status[data-status='insufficient_valid_samples'] { background: #fff2df; color: #8a5b00; }
.candidate-diagnostics { display: grid; grid-template-columns: 1fr; gap: 0; padding: 0 var(--space-4) var(--space-2); }
.candidate-diagnostics div { display: grid; grid-template-columns: 1fr auto; padding: 6px 0; border-bottom: 1px solid #eef0ec; font-size: var(--text-md); }
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
.state-traces { margin: 0 var(--space-4) var(--space-3); }
.state-traces .muted-note { margin: var(--space-2) 0; color: var(--muted); font-size: var(--text-xs); line-height: 1.5; }
.state-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(232px, 1fr)); gap: var(--space-2); }
.optimize-panel { border: 0; background: #f4f6f1; }
/*
 * The rail carries the comparison and the pane carries the reading.
 *
 * Every candidate's with/without/change is visible at once, which is the
 * question a scenario exists to answer, while the charts and diagnostics that
 * explain one of them get the room they need instead of being cropped into a
 * card grid.
 */
.optimize-workspace { display: grid; grid-template-columns: minmax(248px, 1fr) 2fr; align-items: start; gap: var(--space-4); margin-top: var(--space-4); }
.candidate-rail { display: grid; align-content: start; gap: var(--space-2); }
.candidate-summary { display: grid; gap: var(--space-2); padding: var(--space-3); border: 1px solid var(--line); border-radius: var(--radius-md); background: white; text-align: left; }
.candidate-summary:hover { border-color: #b6c6d8; }
.candidate-summary.selected { border-color: #285c91; box-shadow: 0 0 0 1px #285c91; }
.candidate-name { display: grid; gap: 1px; }
.candidate-name strong { font-size: var(--text-md); }
.candidate-name small { color: var(--muted); font: var(--text-2xs) var(--mono); }
.summary-objective { display: grid; gap: 1px; padding-top: var(--space-2); border-top: 1px solid var(--line); }
.summary-outcome { color: var(--muted); font-size: var(--text-2xs); }
.summary-shift { display: flex; align-items: center; gap: 5px; color: var(--ink); font: var(--text-2xs) var(--mono); }
.summary-shift svg { flex: none; color: var(--muted); }
.summary-change { font-size: var(--text-xs); font-weight: 650; }
.summary-change[data-impact='positive'] { color: #277445; }
.summary-change[data-impact='negative'] { color: #a34335; }
.summary-change[data-impact='neutral'] { color: var(--muted); }
.candidate-detail article { overflow: hidden; border: 1px solid var(--line); border-radius: var(--radius-lg); background: white; }
.detail-header { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--space-2); padding: var(--space-4) var(--space-4) var(--space-2); }
.detail-header h3 { margin: 0; font-size: var(--text-xl); }
@media (max-width: 900px) {
  .optimize-workspace { grid-template-columns: minmax(0, 1fr); }
}
</style>
