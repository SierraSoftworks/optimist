<script setup lang="ts">
import { computed } from 'vue'
import { AlertTriangle, GitPullRequestArrow, RefreshCw, RotateCcw } from '@lucide/vue'
import type { EdgeIdentity, StructuralAnalysis } from '../api/types'

const props = defineProps<{
  analysis: StructuralAnalysis | undefined
  pending: boolean
  error: Error | null
  selectedCycle: number | null
}>()
const emit = defineEmits<{
  select: [index: number, nodes: string[], edges: EdgeIdentity[]]
  clear: []
  retry: []
}>()
const feedbackComponents = computed(() =>
  props.analysis?.components.filter((component) => component.is_feedback) ?? [],
)
</script>

<template>
  <aside class="analysis-panel" aria-label="Feedback analysis">
    <header class="analysis-panel-header">
      <div><span class="eyebrow">Exact topology</span><h2>Feedback loops</h2></div>
      <button v-if="selectedCycle !== null" type="button" class="icon-button" title="Clear loop selection" aria-label="Clear loop selection" @click="emit('clear')"><RotateCcw :size="15" /></button>
    </header>

    <div v-if="pending" class="analysis-state"><RefreshCw class="spin" :size="20" /><span>Analyzing causal structure</span></div>
    <div v-else-if="error" class="analysis-state analysis-error">
      <AlertTriangle :size="20" />
      <strong>Analysis unavailable</strong>
      <span>{{ error.message }}</span>
      <button type="button" class="secondary-button" @click="emit('retry')">Retry</button>
    </div>
    <template v-else-if="analysis">
      <div class="analysis-summary">
        <div><strong>{{ feedbackComponents.length }}</strong><span>feedback components</span></div>
        <div><strong>{{ analysis.cycles.length }}</strong><span>elementary cycles</span></div>
        <div><strong>g{{ analysis.revision.graph_revision }}</strong><span>graph revision</span></div>
      </div>
      <div v-if="analysis.cycles_truncated" class="analysis-warning">
        <AlertTriangle :size="15" />
        <span>Cycle results reached the {{ analysis.limits.maximum_cycles }}-cycle limit. Review this as a partial result.</span>
      </div>
      <ol v-if="analysis.cycles.length" class="cycle-list">
        <li v-for="(cycle, index) in analysis.cycles" :key="cycle.edges.map((edge) => `${edge.source}-${edge.kind}-${edge.destination}`).join('|')">
          <button type="button" :aria-pressed="selectedCycle === index" @click="emit('select', index, cycle.nodes, cycle.edges)">
            <span class="cycle-number">{{ index + 1 }}</span>
            <span class="cycle-route">
              <strong>{{ [...cycle.nodes, cycle.nodes[0]].join(' → ') }}</strong>
              <small>{{ cycle.edges.length }} causal relationship{{ cycle.edges.length === 1 ? '' : 's' }}</small>
            </span>
            <GitPullRequestArrow :size="16" />
          </button>
        </li>
      </ol>
      <div v-else class="analysis-empty">
        <GitPullRequestArrow :size="22" />
        <strong>No causal feedback loops</strong>
        <span>The current contributes, changes, and blocks relationships form an acyclic graph.</span>
      </div>
      <section v-if="feedbackComponents.length" class="component-list">
        <h3>Feedback components</h3>
        <div v-for="component in feedbackComponents" :key="component.nodes.join('-')">
          <strong>{{ component.nodes.join(', ') }}</strong>
          <span>{{ component.edges.length }} internal causal edge{{ component.edges.length === 1 ? '' : 's' }}</span>
        </div>
      </section>
    </template>
  </aside>
</template>

<style scoped>
.analysis-warning { display: grid; grid-template-columns: auto 1fr; gap: 9px; margin-top: var(--space-3); padding: var(--space-3); border: 1px solid var(--danger-line); border-radius: var(--radius-md); background: var(--danger-surface); color: #654b46; font-size: var(--text-sm); line-height: 1.5; }
.analysis-warning svg { color: #a83f31; }
.cycle-list { margin: var(--space-4) 0 0; padding: 0; list-style: none; display: grid; gap: 6px; }
.cycle-list button { width: 100%; min-height: 50px; display: grid; grid-template-columns: 24px minmax(0, 1fr) auto; align-items: center; gap: 10px; padding: var(--space-2) var(--space-3); border: 1px solid var(--line); border-radius: var(--radius-md); background: white; color: var(--ink); text-align: left; }
.cycle-list button:hover { background: #f0f3ee; }
.cycle-list button[aria-pressed='true'] { border-color: #a83f31; background: #fff2ef; }
.cycle-route { min-width: 0; display: grid; gap: 3px; }
.cycle-route strong { overflow: hidden; text-overflow: ellipsis; color: var(--ink); font: var(--text-sm) var(--mono); white-space: nowrap; }
.cycle-route small { color: var(--muted); font-size: var(--text-xs); }
.component-list { margin-top: var(--space-5); padding-top: var(--space-4); border-top: 1px solid var(--line); }
.component-list h3 { margin: 0 0 var(--space-2); font-size: var(--text-xs); text-transform: uppercase; letter-spacing: .07em; color: #7b837e; }
.component-list > div { display: grid; gap: 3px; padding: var(--space-2) 0; border-bottom: 1px solid #e5e8e2; }
.component-list strong { font: var(--text-sm) var(--mono); }
.component-list span { color: var(--muted); font-size: var(--text-xs); }
</style>
