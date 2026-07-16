<script setup lang="ts">
import { computed } from 'vue'
import { Activity, Gauge, Goal, Wrench } from '@lucide/vue'
import type { GraphEdge, GraphNode } from '../api/types'

const props = defineProps<{ node: GraphNode | null; edges: GraphEdge[] }>()

const incidentEdges = computed(() =>
  props.node
    ? props.edges.filter(
        (edge) => edge.source === props.node?.id || edge.destination === props.node?.id,
      )
    : [],
)

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

      <section v-if="node.payload.kind === 'outcome' || node.payload.kind === 'factor'" class="inspector-section">
        <h3>State estimates</h3>
        <dl>
          <div><dt>Current</dt><dd>{{ distribution(node, 'current')?.type ?? 'Not set' }}</dd></div>
          <div><dt>Desired</dt><dd>{{ distribution(node, 'desired')?.type ?? 'Not set' }}</dd></div>
          <div v-if="node.payload.kind === 'factor'"><dt>Controllable</dt><dd>{{ node.payload.properties.controllable ? 'Yes' : 'No' }}</dd></div>
          <div v-if="node.payload.kind === 'outcome'"><dt>Direction</dt><dd>{{ node.payload.properties.direction }}</dd></div>
        </dl>
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
          <li v-for="(edge, index) in incidentEdges" :key="`${edge.source}-${edge.destination}-${index}`">
            <span>{{ edge.source }}</span>
            <strong>{{ edge.payload.kind.replaceAll('_', ' ') }}</strong>
            <span>{{ edge.destination }}</span>
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
