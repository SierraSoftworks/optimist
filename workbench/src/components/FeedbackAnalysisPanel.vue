<script setup lang="ts">
import { computed } from 'vue'
import { AlertTriangle, GitPullRequestArrow, RefreshCw, RotateCcw } from '@lucide/vue'
import type { EdgeIdentity, FeedbackLoop, GraphNode, StructuralAnalysis } from '../api/types'

const props = defineProps<{
  analysis: StructuralAnalysis | undefined
  loops: FeedbackLoop[]
  nodes: GraphNode[]
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
const nodeTitles = computed(() => new Map(props.nodes.map((node) => [node.id, node.title])))
/**
 * Both surfaces enumerate circuits through the same code with the same canonical
 * rotation, so a cycle's member list identifies it in either.
 */
const weighed = computed(
  () => new Map(props.loops.map((loop) => [loop.states.join('>'), loop])),
)

function title(id: string) {
  return nodeTitles.value.get(id) ?? id
}

function loopFor(nodes: string[]) {
  return weighed.value.get(nodes.join('>')) ?? null
}

function gainLabel(loop: FeedbackLoop) {
  if (loop.gain === null) return 'gain unmeasurable'
  return `gain ${Math.abs(loop.gain) < 0.005 ? loop.gain.toExponential(1) : loop.gain.toFixed(2)}`
}

/** A loop settles only when its gain is known, contracts, and rarely crosses one. */
function tone(loop: FeedbackLoop) {
  if (loop.gain === null) return 'unknown'
  if (Math.abs(loop.gain) >= 1 || (loop.instability ?? 0) >= 0.05) return 'amplifying'
  return 'damping'
}

/**
 * Bar width for one hop's share of the compounding.
 *
 * Shares are logarithms and can span orders of magnitude, so they are scaled
 * against the largest on the loop rather than against a fixed range; the
 * question is which relationship dominates, not what its logarithm is.
 */
function share(loop: FeedbackLoop, contribution: number) {
  const largest = Math.max(...loop.weights.map((weight) => Math.abs(weight.contribution)), 1e-9)
  return `${Math.max(4, (Math.abs(contribution) / largest) * 100)}%`
}
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
            <span v-if="loopFor(cycle.nodes)" class="loop-gain" :data-tone="tone(loopFor(cycle.nodes)!)">
              {{ gainLabel(loopFor(cycle.nodes)!) }}
            </span>
            <GitPullRequestArrow v-else :size="16" />
          </button>
          <div v-if="loopFor(cycle.nodes)?.weights.length" class="loop-weights">
            <p class="loop-note">
              <template v-if="tone(loopFor(cycle.nodes)!) === 'amplifying'">
                A deviation grows each trip around this loop.
              </template>
              <template v-else>
                A deviation shrinks each trip around this loop.
              </template>
              Each relationship's share of that compounding:
            </p>
            <div
              v-for="weight in loopFor(cycle.nodes)!.weights"
              :key="`${weight.source}-${weight.destination}`"
              class="weight-row"
              :data-amplifies="weight.contribution > 0"
            >
              <span class="weight-name">{{ title(weight.source) }} → {{ title(weight.destination) }}</span>
              <span class="weight-bar"><i :style="{ width: share(loopFor(cycle.nodes)!, weight.contribution) }"></i></span>
              <code>{{ weight.response.toFixed(3) }}</code>
            </div>
            <p v-if="loopFor(cycle.nodes)!.instability" class="loop-note negative">
              Runs away in {{ ((loopFor(cycle.nodes)!.instability ?? 0) * 100).toFixed(0) }}% of sampled draws.
            </p>
          </div>
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
.loop-gain { flex: none; padding: 4px 8px; border-radius: var(--radius-sm); background: #edf0eb; color: var(--muted); font: var(--text-xs) var(--mono); font-weight: 700; white-space: nowrap; }
.loop-gain[data-tone='amplifying'] { background: var(--danger-surface); color: var(--danger); }
.loop-gain[data-tone='damping'] { background: var(--green-soft); color: var(--green); }
.loop-gain[data-tone='unknown'] { background: var(--caution-surface); color: var(--caution); }
.loop-weights { padding: var(--space-2) var(--space-3) var(--space-3); border-top: 1px solid var(--line); }
.loop-note { margin: 0 0 var(--space-2); color: var(--muted); font-size: var(--text-xs); line-height: 1.5; }
.loop-note.negative { margin: var(--space-2) 0 0; color: var(--danger); }
.weight-row { display: grid; grid-template-columns: minmax(0, 1fr) 72px auto; align-items: center; gap: var(--space-2); padding: 3px 0; font-size: var(--text-xs); }
.weight-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.weight-bar { height: 6px; border-radius: 3px; background: #e7eae4; overflow: hidden; }
.weight-bar i { display: block; height: 100%; background: #7ea88e; }
.weight-row[data-amplifies='true'] .weight-bar i { background: #c0705f; }
.weight-row code { color: var(--muted); font: var(--text-xs) var(--mono); }
.weight-row[data-amplifies='true'] code { color: var(--danger); font-weight: 700; }
.component-list { margin-top: var(--space-5); padding-top: var(--space-4); border-top: 1px solid var(--line); }
.component-list h3 { margin: 0 0 var(--space-2); font-size: var(--text-xs); text-transform: uppercase; letter-spacing: .07em; color: #7b837e; }
.component-list > div { display: grid; gap: 3px; padding: var(--space-2) 0; border-bottom: 1px solid #e5e8e2; }
.component-list strong { font: var(--text-sm) var(--mono); }
.component-list span { color: var(--muted); font-size: var(--text-xs); }
</style>
