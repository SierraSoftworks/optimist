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
.readiness-panel { border: 0; padding: 24px clamp(18px, 4vw, 52px); background: #f4f6f1; }
.readiness-header { display: grid; grid-template-columns: 1fr minmax(280px, 520px); align-items: end; gap: 24px; }
.readiness-header h2 { font-size: 24px; }
.readiness-header p { margin: 0; color: var(--muted); font-size: 12px; line-height: 1.55; }
.readiness-summary { display: grid; grid-template-columns: repeat(4, 1fr); gap: 8px; margin-top: 22px; }
.readiness-summary div { display: grid; gap: 3px; padding: 12px; border: 1px solid var(--line); background: white; }
.readiness-summary strong { font: 18px 'IBM Plex Mono', monospace; }
.readiness-summary span { color: var(--muted); font-size: 9px; }
.readiness-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(360px, 1fr)); gap: 14px; margin-top: 14px; }
.readiness-card { min-width: 0; border: 1px solid var(--line); background: white; }
.readiness-card > header { display: grid; grid-template-columns: 28px minmax(0, 1fr) auto; align-items: center; gap: 9px; padding: 12px; border-bottom: 1px solid var(--line); }
.priority { display: grid; width: 26px; height: 26px; place-items: center; background: #e7ece7; font: 11px 'IBM Plex Mono', monospace; }
.readiness-card h3 { margin: 0; font-size: 14px; }
.readiness-card small { color: var(--muted); font: 8px 'IBM Plex Mono', monospace; }
.readiness-badge { display: flex; align-items: center; gap: 4px; padding: 4px 6px; background: #fff0e8; color: #8c4336; font-size: 8px; font-weight: 700; text-transform: uppercase; }
.readiness-badge[data-ready='true'] { background: var(--green-soft); color: var(--green); }
.combined-metrics { display: grid; grid-template-columns: 1fr 1fr; border-bottom: 1px solid var(--line); }
.combined-metrics div { display: grid; grid-template-columns: auto 1fr; gap: 2px 6px; padding: 10px 12px; }
.combined-metrics div + div { border-left: 1px solid var(--line); }
.combined-metrics svg { grid-row: span 2; color: var(--green); }
.combined-metrics span { color: var(--muted); font-size: 8px; }
.combined-metrics strong { font: 11px 'IBM Plex Mono', monospace; }
.execution-plan, .blocker-list { padding: 12px; }
.execution-plan h4, .blocker-list h4 { display: flex; align-items: center; gap: 5px; margin: 0 0 8px; font-size: 10px; }
.execution-plan ol { margin: 0; padding: 0; list-style: none; display: grid; gap: 8px; }
.execution-plan li { display: grid; gap: 7px; padding: 9px; border-left: 3px solid #7ba08b; background: #f8faf7; }
.step-title { display: flex; justify-content: space-between; gap: 8px; }
.step-title strong { font-size: 10px; }
.step-title span { color: var(--muted); font-size: 8px; }
.blocker-list { border-top: 1px solid var(--line); background: #fff9ee; }
.blocker-list p { display: flex; justify-content: space-between; gap: 8px; margin: 4px 0; font-size: 9px; }
.blocker-list p span { color: #765b27; }
.readiness-card > footer { display: flex; flex-wrap: wrap; gap: 6px; padding: 9px 12px; border-top: 1px solid var(--line); }
.readiness-card > footer span { display: flex; align-items: center; gap: 4px; padding: 4px 6px; font-size: 8px; }
.synergy { background: #edf5ee; color: #376c4c; }
.conflict { background: #fff0e8; color: #8c4336; }
@media (max-width: 720px) {
  .readiness-header { grid-template-columns: 1fr; }
  .readiness-summary { grid-template-columns: 1fr 1fr; }
  .readiness-grid { grid-template-columns: 1fr; }
}
</style>
