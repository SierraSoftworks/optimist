<script setup lang="ts">
import { computed, ref } from 'vue'
import { AlertTriangle, GitBranch, RefreshCw, ShieldCheck } from '@lucide/vue'
import type { EdgeIdentity, ImpedimentAnalysis, ImpedimentCandidate, GraphNode } from '../api/types'

const props = defineProps<{
  analysis: ImpedimentAnalysis | undefined
  pending: boolean
  error: Error | null
  nodes: GraphNode[]
  selectedFactorId: string | null
}>()
const emit = defineEmits<{
  select: [factor: string, nodes: string[], edges: EdgeIdentity[]]
  retry: []
}>()
const order = ref<'topology' | 'evidence'>('topology')
const nodeTitles = computed(() => new Map(props.nodes.map((node) => [node.id, node.title])))
const candidatesById = computed(() => new Map(
  props.analysis?.topology_candidates.map((candidate) => [candidate.factor, candidate]) ?? [],
))
const candidates = computed(() => {
  if (!props.analysis) return []
  if (order.value === 'topology') return props.analysis.topology_candidates
  return props.analysis.evidence_priority
    .map((id) => candidatesById.value.get(id))
    .filter((candidate): candidate is ImpedimentCandidate => Boolean(candidate))
})

function title(id: string) {
  return nodeTitles.value.get(id) ?? id
}

function evidenceReferenceCount(candidate: ImpedimentCandidate) {
  return candidate.relationship_evidence.reduce(
    (total, value) => total + value.references.length,
    0,
  )
}

function select(candidate: ImpedimentCandidate) {
  emit('select', candidate.factor, [candidate.factor, ...candidate.reachable_outcomes], candidate.path_edges)
}
</script>

<template>
  <aside class="analysis-panel impediment-panel" aria-label="Impediments analysis">
    <header class="analysis-panel-header">
      <div><span class="eyebrow">Review candidates</span><h2>Impediments</h2></div>
    </header>
    <div v-if="pending" class="analysis-state"><RefreshCw class="spin" :size="20" /><span>Tracing factor-to-outcome paths</span></div>
    <div v-else-if="error" class="analysis-state analysis-error">
      <AlertTriangle :size="20" /><strong>Analysis unavailable</strong><span>{{ error.message }}</span>
      <button type="button" class="secondary-button" @click="emit('retry')">Retry</button>
    </div>
    <template v-else-if="analysis">
      <div class="analysis-summary">
        <div><strong>{{ analysis.topology_candidates.length }}</strong><span>candidate factors</span></div>
        <div><strong>{{ analysis.topology_candidates.filter((candidate) => candidate.controllable).length }}</strong><span>controllable</span></div>
        <div><strong>g{{ analysis.revision.graph_revision }}</strong><span>graph revision</span></div>
      </div>
      <div class="analysis-order-tabs" role="group" aria-label="Impediment ordering">
        <button type="button" :aria-pressed="order === 'topology'" @click="order = 'topology'"><GitBranch :size="13" /> Topology</button>
        <button type="button" :aria-pressed="order === 'evidence'" @click="order = 'evidence'"><ShieldCheck :size="13" /> Evidence</button>
      </div>
      <p class="analysis-boundary">Topology orders by outcome reach and shortest path. Evidence order prioritizes documented factors and path edges separately. Neither is a causal confidence score.</p>
      <ol v-if="candidates.length" class="impediment-list">
        <li v-for="(candidate, index) in candidates" :key="candidate.factor">
          <button type="button" :aria-pressed="selectedFactorId === candidate.factor" @click="select(candidate)">
            <span class="cycle-number">{{ index + 1 }}</span>
            <span class="impediment-title"><strong>{{ title(candidate.factor) }}</strong><small>{{ candidate.factor }} · {{ candidate.controllable ? 'controllable' : 'not directly controllable' }}</small></span>
          </button>
          <dl class="impediment-facts">
            <div><dt>Reachable outcomes</dt><dd>{{ candidate.reachable_outcomes.length }}</dd></div>
            <div><dt>Nearest outcome</dt><dd>{{ candidate.nearest_outcome_distance }} edge{{ candidate.nearest_outcome_distance === 1 ? '' : 's' }}</dd></div>
            <div><dt>Direct evidence</dt><dd>{{ candidate.direct_evidence.length }}</dd></div>
            <div><dt>Path references</dt><dd>{{ evidenceReferenceCount(candidate) }}</dd></div>
          </dl>
          <div v-if="candidate.unsupported_path_edges.length" class="unsupported-path">
            <AlertTriangle :size="13" /><span>{{ candidate.unsupported_path_edges.length }} path edge{{ candidate.unsupported_path_edges.length === 1 ? '' : 's' }} lack{{ candidate.unsupported_path_edges.length === 1 ? 's' : '' }} typed evidence.</span>
          </div>
          <p class="reachable-outcomes">Outcomes: {{ candidate.reachable_outcomes.map(title).join(', ') }}</p>
        </li>
      </ol>
      <div v-else class="analysis-empty">
        <GitBranch :size="22" /><strong>No impediment candidates</strong><span>Add causal factor-to-outcome paths before reviewing impediments.</span>
      </div>
    </template>
  </aside>
</template>

<style scoped>
.analysis-order-tabs { display: grid; grid-template-columns: 1fr 1fr; gap: 4px; margin-top: 12px; padding: 3px; border: 1px solid var(--line); border-radius: 5px; background: white; }
.analysis-order-tabs button { min-height: 28px; display: flex; align-items: center; justify-content: center; gap: 5px; border: 0; border-radius: 3px; background: transparent; color: var(--muted); font-size: 9px; font-weight: 700; }
.analysis-order-tabs button[aria-pressed='true'] { background: var(--green-soft); color: var(--green); }
.impediment-list { margin: 12px 0 0; padding: 0; list-style: none; display: grid; gap: 8px; }
.impediment-list > li { overflow: hidden; border: 1px solid var(--line); border-radius: 6px; background: white; }
.impediment-list > li > button { width: 100%; display: grid; grid-template-columns: 24px minmax(0, 1fr); gap: 8px; align-items: center; padding: 8px; border: 0; background: transparent; text-align: left; }
.impediment-list > li > button:hover, .impediment-list > li > button[aria-pressed='true'] { background: #edf3f9; }
.impediment-list > li > button[aria-pressed='true'] { box-shadow: inset 3px 0 #285c91; }
.impediment-title { min-width: 0; display: grid; gap: 2px; }
.impediment-title strong { overflow: hidden; text-overflow: ellipsis; font-size: 10px; white-space: nowrap; }
.impediment-title small { color: var(--muted); font: 8px 'IBM Plex Mono', monospace; }
.impediment-facts { grid-template-columns: 1fr; gap: 0; padding: 0 9px 7px; }
.impediment-facts div { grid-template-columns: 1fr auto; padding: 3px 0; border-bottom: 1px solid #eef0ec; font-size: 8px; }
.unsupported-path { display: grid; grid-template-columns: auto 1fr; gap: 6px; margin: 0 8px 7px; padding: 6px; border-radius: 4px; background: #fff2df; color: #765b27; font-size: 8px; line-height: 1.4; }
.reachable-outcomes { margin: 0; padding: 7px 9px; border-top: 1px solid var(--line); color: var(--muted); font-size: 8px; line-height: 1.4; }
</style>
