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
