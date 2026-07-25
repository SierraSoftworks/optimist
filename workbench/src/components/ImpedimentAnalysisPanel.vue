<script setup lang="ts">
import { computed } from 'vue'
import { AlertTriangle, CheckCircle2, Clock3, GitBranch, RefreshCw, Sparkles } from '@lucide/vue'
import type { GraphNode, ImpedimentAnalysis } from '../api/types'
import DistributionStrip from './DistributionStrip.vue'

const props = defineProps<{
  analysis: ImpedimentAnalysis | undefined
  pending: boolean
  error: Error | null
  nodes: GraphNode[]
}>()
const emit = defineEmits<{ retry: [] }>()
const nodeTitles = computed(() => new Map(props.nodes.map((node) => [node.id, node.title])))

function title(id: string) {
  return nodeTitles.value.get(id) ?? id
}

function hardBlockers(candidate: ImpedimentAnalysis['candidates'][number]) {
  return candidate.blocking_requirements.filter((requirement) => requirement.hard)
}
</script>

<template>
  <main class="analysis-panel readiness-panel" aria-label="Impediments analysis">
    <header class="readiness-header">
      <div><span class="eyebrow">Execution readiness</span><h2>Intervention impediments</h2></div>
      <p>Required interventions run first. Their durations add and every required step must succeed before the candidate can complete.</p>
    </header>
    <div v-if="pending" class="analysis-state"><RefreshCw class="spin" :size="20" /><span>Building intervention dependency plans</span></div>
    <div v-else-if="error" class="analysis-state analysis-error">
      <AlertTriangle :size="20" /><strong>Readiness unavailable</strong><span>{{ error.message }}</span>
      <button type="button" class="secondary-button" @click="emit('retry')">Retry</button>
    </div>
    <template v-else-if="analysis">
      <div class="readiness-summary">
        <div><strong>{{ analysis.candidates.length }}</strong><span>interventions</span></div>
        <div><strong>{{ analysis.candidates.filter((candidate) => !hardBlockers(candidate).length).length }}</strong><span>without hard blockers</span></div>
        <div><strong>{{ analysis.candidates.filter((candidate) => candidate.execution_steps.length > 1).length }}</strong><span>dependency plans</span></div>
        <div><strong>{{ analysis.candidates.filter((candidate) => candidate.synergies.length).length }}</strong><span>with synergies</span></div>
      </div>
      <section v-if="analysis.candidates.length" class="readiness-grid">
        <article v-for="(candidate, index) in analysis.candidates" :key="candidate.intervention" class="readiness-card">
          <header>
            <span class="priority">{{ index + 1 }}</span>
            <div><h3>{{ title(candidate.intervention) }}</h3><small>{{ candidate.intervention }}</small></div>
            <span class="readiness-badge" :data-ready="!hardBlockers(candidate).length">
              <CheckCircle2 v-if="!hardBlockers(candidate).length" :size="13" />
              <AlertTriangle v-else :size="13" />
              {{ hardBlockers(candidate).length ? `${hardBlockers(candidate).length} blocked` : 'Executable' }}
            </span>
          </header>
          <div class="combined-metrics">
            <div><Clock3 :size="14" /><span>Total expected duration</span><strong>{{ Number(candidate.expected_duration.toPrecision(3)) }} periods</strong></div>
            <div><CheckCircle2 :size="14" /><span>Plan success</span><strong>{{ (candidate.expected_success_probability * 100).toFixed(1) }}%</strong></div>
          </div>
          <section class="execution-plan">
            <h4><GitBranch :size="13" /> Execution order</h4>
            <ol>
              <li v-for="step in candidate.execution_steps" :key="step.intervention">
                <div class="step-title"><strong>{{ title(step.intervention) }}</strong><span>{{ step.intervention === candidate.intervention ? 'Candidate' : 'Required first' }}</span></div>
                <DistributionStrip :distribution="step.duration" kind="duration" />
                <DistributionStrip :distribution="step.probability_of_success" kind="probability" />
              </li>
            </ol>
          </section>
          <section v-if="candidate.blocking_requirements.length" class="blocker-list">
            <h4><AlertTriangle :size="13" /> Factor requirements</h4>
            <p v-for="requirement in candidate.blocking_requirements" :key="`${requirement.dependent}-${requirement.prerequisite}`">
              <strong>{{ title(requirement.prerequisite) }}</strong>
              <span>{{ requirement.hard ? 'Hard blocker' : 'Soft requirement' }}<template v-if="requirement.satisfaction_threshold !== null"> · threshold {{ requirement.satisfaction_threshold }}</template></span>
            </p>
          </section>
          <footer v-if="candidate.synergies.length || candidate.conflicts.length">
            <span v-if="candidate.synergies.length" class="synergy"><Sparkles :size="12" /> Synergy: {{ candidate.synergies.map(title).join(', ') }}</span>
            <span v-if="candidate.conflicts.length" class="conflict"><AlertTriangle :size="12" /> Conflicts: {{ candidate.conflicts.map(title).join(', ') }}</span>
          </footer>
        </article>
      </section>
      <div v-else class="analysis-empty"><GitBranch :size="22" /><strong>No interventions</strong><span>Add interventions and Requires relationships to review execution readiness.</span></div>
    </template>
  </main>
</template>

<style scoped>
.readiness-panel { border: 0; background: #f4f6f1; }
.readiness-header { display: grid; grid-template-columns: 1fr minmax(280px, 520px); align-items: end; gap: var(--space-5); }
.readiness-header h2 { font-size: var(--text-2xl); }
.readiness-header p { margin: 0; color: var(--muted); font-size: var(--text-md); line-height: 1.6; }
.readiness-summary { display: grid; grid-template-columns: repeat(auto-fit, minmax(140px, 1fr)); gap: var(--space-2); margin-top: var(--space-5); }
.readiness-summary div { display: grid; gap: 3px; padding: var(--space-3); border: 1px solid var(--line); border-radius: var(--radius-md); background: white; }
.readiness-summary strong { font: var(--text-xl) var(--mono); }
.readiness-summary span { color: var(--muted); font-size: var(--text-xs); }
/* Cards widen before they multiply, so a comparison stays readable at any width. */
.readiness-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 460px), 1fr)); gap: var(--space-3); margin-top: var(--space-4); }
.readiness-card { min-width: 0; border: 1px solid var(--line); border-radius: var(--radius-lg); overflow: hidden; background: white; }
.readiness-card > header { display: grid; grid-template-columns: 28px minmax(0, 1fr) auto; align-items: center; gap: 10px; padding: var(--space-3) var(--space-4); border-bottom: 1px solid var(--line); }
.priority { display: grid; width: 28px; height: 28px; place-items: center; border-radius: var(--radius-sm); background: #e7ece7; font: var(--text-sm) var(--mono); }
.readiness-card h3 { margin: 0; font-size: var(--text-lg); }
.readiness-card small { color: var(--muted); font: var(--text-2xs) var(--mono); }
.readiness-badge { display: flex; align-items: center; gap: 5px; padding: 5px 8px; border-radius: var(--radius-sm); background: #fff0e8; color: #8c4336; font-size: var(--text-2xs); font-weight: 700; text-transform: uppercase; letter-spacing: .04em; }
.readiness-badge[data-ready='true'] { background: var(--green-soft); color: var(--green); }
.combined-metrics { display: grid; grid-template-columns: 1fr 1fr; border-bottom: 1px solid var(--line); }
.combined-metrics div { display: grid; grid-template-columns: auto 1fr; gap: 2px 8px; padding: var(--space-3) var(--space-4); }
.combined-metrics div + div { border-left: 1px solid var(--line); }
.combined-metrics svg { grid-row: span 2; color: var(--green); }
.combined-metrics span { color: var(--muted); font-size: var(--text-xs); }
.combined-metrics strong { font: var(--text-md) var(--mono); }
.execution-plan, .blocker-list { padding: var(--space-3) var(--space-4); }
.execution-plan h4, .blocker-list h4 { display: flex; align-items: center; gap: 6px; margin: 0 0 var(--space-2); font-size: var(--text-xs); text-transform: uppercase; letter-spacing: .06em; color: #7b837e; }
.execution-plan ol { margin: 0; padding: 0; list-style: none; display: grid; gap: var(--space-2); }
.execution-plan li { display: grid; gap: 7px; padding: var(--space-3); border-left: 3px solid #7ba08b; border-radius: 0 var(--radius-sm) var(--radius-sm) 0; background: #f8faf7; }
.step-title { display: flex; justify-content: space-between; gap: var(--space-2); }
.step-title strong { font-size: var(--text-sm); }
.step-title span { color: var(--muted); font-size: var(--text-xs); }
.blocker-list { border-top: 1px solid var(--line); background: #fff9ee; }
.blocker-list p { display: flex; justify-content: space-between; gap: var(--space-2); margin: 5px 0; font-size: var(--text-sm); }
.blocker-list p span { color: #765b27; }
.readiness-card > footer { display: flex; flex-wrap: wrap; gap: 6px; padding: var(--space-3) var(--space-4); border-top: 1px solid var(--line); }
.readiness-card > footer span { display: flex; align-items: center; gap: 5px; padding: 5px 8px; border-radius: var(--radius-sm); font-size: var(--text-xs); }
.synergy { background: #edf5ee; color: #376c4c; }
.conflict { background: #fff0e8; color: #8c4336; }
@media (max-width: 720px) {
  .readiness-header { grid-template-columns: 1fr; }
  .readiness-grid { grid-template-columns: 1fr; }
}
</style>
