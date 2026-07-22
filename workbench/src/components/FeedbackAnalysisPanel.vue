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
.analysis-warning { display: grid; grid-template-columns: auto 1fr; gap: 7px; margin-top: 10px; padding: 8px; border: 1px solid #d8a098; border-radius: 5px; background: #fff8f6; color: #654b46; font-size: 9px; line-height: 1.45; }
.analysis-warning svg { color: #a83f31; }
.cycle-list { margin: 14px 0 0; padding: 0; list-style: none; display: grid; gap: 6px; }
.cycle-list button { width: 100%; min-height: 48px; display: grid; grid-template-columns: 24px minmax(0, 1fr) auto; align-items: center; gap: 8px; padding: 7px; border: 1px solid var(--line); border-radius: 5px; background: white; color: var(--ink); text-align: left; }
.cycle-list button:hover { background: #f0f3ee; }
.cycle-list button[aria-pressed='true'] { border-color: #a83f31; background: #fff2ef; }
.cycle-route { min-width: 0; display: grid; gap: 3px; }
.cycle-route strong { overflow: hidden; text-overflow: ellipsis; color: var(--ink); font: 10px 'IBM Plex Mono', monospace; white-space: nowrap; }
.cycle-route small { color: var(--muted); font-size: 8px; }
.component-list { margin-top: 18px; padding-top: 14px; border-top: 1px solid var(--line); }
.component-list h3 { margin: 0 0 8px; font-size: 10px; text-transform: uppercase; letter-spacing: .06em; }
.component-list > div { display: grid; gap: 2px; padding: 7px 0; border-bottom: 1px solid #e5e8e2; }
.component-list strong { font: 9px 'IBM Plex Mono', monospace; }
.component-list span { color: var(--muted); font-size: 8px; }
</style>
