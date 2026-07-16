<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Activity, Gauge, Goal, Pencil, Sigma, Trash2, Wrench } from '@lucide/vue'
import type { GraphEdge, GraphNode } from '../api/types'

const props = defineProps<{ node: GraphNode | null; edges: GraphEdge[] }>()
const emit = defineEmits<{ edit: []; estimate: []; relationship: [edge: GraphEdge]; delete: [] }>()
const confirmDelete = ref(false)

const incidentEdges = computed(() =>
  props.node
    ? props.edges.filter(
        (edge) => edge.source === props.node?.id || edge.destination === props.node?.id,
      )
    : [],
)
watch(() => props.node?.id, () => { confirmDelete.value = false })

const kindLabel = computed(() => props.node?.payload.kind ?? '')
const Icon = computed(() => {
  switch (props.node?.payload.kind) {
    case 'outcome':
      return Goal
    case 'metric':
      return Gauge
    case 'intervention':
      return Wrench
    default:
      return Activity
  }
})

function distribution(node: GraphNode, slot: 'current' | 'desired') {
  if (node.payload.kind !== 'outcome' && node.payload.kind !== 'factor') return null
  return node.payload.properties[slot]?.distribution ?? null
}

function distributionLabel(node: GraphNode, slot: 'current' | 'desired') {
  const value = distribution(node, slot)
  if (!value) return 'Not set'
  if (value.type === 'point') return `Point · ${value.value}`
  if (value.type === 'beta') return `Beta · α ${value.alpha}, β ${value.beta}`
  if (value.type === 'scaled_beta') {
    return `Scaled Beta · [${value.lower}, ${value.upper}]`
  }
  if (value.type === 'normal') return `Normal · μ ${value.mean}, σ ${value.standard_deviation}`
  return `LogNormal · μ ${value.location}, σ ${value.scale}`
}

function provenance(node: GraphNode, slot: 'current' | 'desired') {
  if (node.payload.kind !== 'outcome' && node.payload.kind !== 'factor') return []
  return node.payload.properties[slot]?.provenance ?? []
}
</script>

<template>
  <aside class="inspector" aria-label="Selection inspector">
    <template v-if="node">
      <header class="inspector-header">
        <span class="kind-icon" :data-kind="node.payload.kind"><component :is="Icon" :size="18" /></span>
        <div>
          <span class="eyebrow">{{ kindLabel }} · {{ node.id }}</span>
          <h2>{{ node.title }}</h2>
        </div>
      </header>

      <div class="inspector-actions">
        <button type="button" class="secondary-button" @click="emit('edit')"><Pencil :size="14" /> Details</button>
        <button
          v-if="node.payload.kind === 'outcome' || node.payload.kind === 'factor'"
          type="button"
          class="secondary-button"
          @click="emit('estimate')"
        ><Sigma :size="14" /> Estimate</button>
      </div>

      <p v-if="node.description" class="description">{{ node.description }}</p>
      <p v-else class="muted">No description has been added.</p>

      <section class="inspector-section">
        <h3>Identity</h3>
        <dl>
          <div><dt>Name</dt><dd>{{ node.name }}</dd></div>
          <div><dt>Revision</dt><dd>{{ node.revision }}</dd></div>
          <div v-if="node.aliases.length"><dt>Aliases</dt><dd>{{ node.aliases.join(', ') }}</dd></div>
        </dl>
      </section>

      <section class="inspector-section danger-section">
        <h3>Delete node</h3>
        <p v-if="incidentEdges.length" class="muted">Delete {{ incidentEdges.length }} connected relationship{{ incidentEdges.length === 1 ? '' : 's' }} first.</p>
        <button
          type="button"
          class="danger-button"
          :disabled="incidentEdges.length > 0"
          @click="confirmDelete ? emit('delete') : (confirmDelete = true)"
        ><Trash2 :size="14" /> {{ confirmDelete ? `Confirm delete ${node.id}` : 'Delete node' }}</button>
      </section>

      <section v-if="node.payload.kind === 'outcome' || node.payload.kind === 'factor'" class="inspector-section">
        <h3>State estimates</h3>
        <dl>
          <div><dt>Current</dt><dd>{{ distributionLabel(node, 'current') }}</dd></div>
          <div v-if="provenance(node, 'current').length"><dt>Current source</dt><dd>{{ provenance(node, 'current').join('; ') }}</dd></div>
          <div><dt>Desired</dt><dd>{{ distributionLabel(node, 'desired') }}</dd></div>
          <div v-if="provenance(node, 'desired').length"><dt>Desired source</dt><dd>{{ provenance(node, 'desired').join('; ') }}</dd></div>
          <div v-if="node.payload.kind === 'factor'"><dt>Controllable</dt><dd>{{ node.payload.properties.controllable ? 'Yes' : 'No' }}</dd></div>
          <div v-if="node.payload.kind === 'outcome'"><dt>Direction</dt><dd>{{ node.payload.properties.direction }}</dd></div>
        </dl>
      </section>

      <section v-if="Object.keys(node.metadata).length" class="inspector-section">
        <h3>Metadata</h3>
        <pre class="metadata-view">{{ JSON.stringify(node.metadata, null, 2) }}</pre>
      </section>

      <section v-if="node.payload.kind === 'metric'" class="inspector-section">
        <h3>Measurement</h3>
        <dl>
          <div><dt>Unit</dt><dd>{{ node.payload.properties.unit }}</dd></div>
          <div><dt>Aggregation</dt><dd>{{ node.payload.properties.aggregation ?? 'Not set' }}</dd></div>
        </dl>
      </section>

      <section v-if="node.payload.kind === 'intervention'" class="inspector-section">
        <h3>Investment</h3>
        <dl>
          <div><dt>Cost dimensions</dt><dd>{{ node.payload.properties.costs.length }}</dd></div>
          <div><dt>Duration</dt><dd>{{ node.payload.properties.duration?.distribution.type ?? 'Not set' }}</dd></div>
          <div><dt>Success prior</dt><dd>{{ node.payload.properties.probability_of_success?.distribution.type ?? 'Not set' }}</dd></div>
        </dl>
      </section>

      <section class="inspector-section">
        <h3>Relationships <span>{{ incidentEdges.length }}</span></h3>
        <ul v-if="incidentEdges.length" class="relationship-list">
          <li v-for="edge in incidentEdges" :key="`${edge.source}-${edge.payload.kind}-${edge.destination}`">
            <button type="button" :aria-label="`Edit ${edge.payload.kind.replaceAll('_', ' ')} relationship ${edge.source} to ${edge.destination}`" @click="emit('relationship', edge)">
              <span>{{ edge.source }}</span>
              <strong>{{ edge.payload.kind.replaceAll('_', ' ') }}</strong>
              <span>{{ edge.destination }}</span>
            </button>
          </li>
        </ul>
        <p v-else class="muted">No connected relationships.</p>
      </section>
    </template>

    <div v-else class="empty-inspector">
      <Activity :size="24" />
      <h2>Nothing selected</h2>
      <p>Select a node in the graph or outline to inspect its typed properties.</p>
    </div>
  </aside>
</template>
