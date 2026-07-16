<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { Core, CytoscapeOptions, ElementDefinition } from 'cytoscape'
import { Focus, Minus, Plus } from '@lucide/vue'
import type { GraphEdge, GraphNode } from '../api/types'

const props = defineProps<{
  nodes: GraphNode[]
  edges: GraphEdge[]
  selectedNodeId: string | null
  highlightedNodeIds?: string[]
  highlightedEdgeIds?: string[]
}>()

const emit = defineEmits<{ select: [id: string | null] }>()
const container = ref<HTMLDivElement>()
let graph: Core | null = null
let resizeObserver: ResizeObserver | null = null
let resizeTimer: ReturnType<typeof setTimeout> | null = null

const graphSignature = computed(() =>
  JSON.stringify({
    nodes: props.nodes.map((node) => [node.id, node.title, node.payload.kind]),
    edges: props.edges.map((edge) => [edge.source, edge.payload.kind, edge.destination]),
  }),
)

type CytoscapeStyle = NonNullable<CytoscapeOptions['style']>

const styles: CytoscapeStyle = [
  {
    selector: 'node',
    style: {
      width: 46,
      height: 46,
      label: 'data(label)',
      'font-family': 'Manrope, sans-serif',
      'font-size': 11,
      'font-weight': 600,
      color: '#25292b',
      'text-wrap': 'wrap',
      'text-max-width': '116px',
      'text-valign': 'bottom',
      'text-margin-y': 10,
      'background-color': '#f5f6f2',
      'border-width': 2,
      'border-color': '#767d78',
      'overlay-opacity': 0,
    },
  },
  {
    selector: 'node[kind = "outcome"]',
    style: { 'background-color': '#f5b83f', 'border-color': '#8a5b00', shape: 'diamond' },
  },
  {
    selector: 'node[kind = "metric"]',
    style: { 'background-color': '#71c6c2', 'border-color': '#176c69', shape: 'round-rectangle' },
  },
  {
    selector: 'node[kind = "factor"]',
    style: { 'background-color': '#8eb6e0', 'border-color': '#285c91', shape: 'ellipse' },
  },
  {
    selector: 'node[kind = "intervention"]',
    style: { 'background-color': '#e89873', 'border-color': '#8e4020', shape: 'hexagon' },
  },
  {
    selector: 'node:selected',
    style: {
      'border-width': 4,
      'border-color': '#121719',
      'underlay-color': '#121719',
      'underlay-opacity': 0.1,
      'underlay-padding': 8,
    },
  },
  {
    selector: 'node.analysis-highlight',
    style: {
      'border-width': 5,
      'border-color': '#a83f31',
      'underlay-color': '#d35a47',
      'underlay-opacity': 0.16,
      'underlay-padding': 11,
    },
  },
  {
    selector: 'edge',
    style: {
      width: 1.5,
      'line-color': '#9ba29d',
      'target-arrow-color': '#68716b',
      'target-arrow-shape': 'triangle',
      'curve-style': 'bezier',
      'arrow-scale': 0.75,
      opacity: 0.8,
    },
  },
  {
    selector: 'edge[kind = "contributes"], edge[kind = "changes"], edge[kind = "blocks"]',
    style: { width: 2.5, 'line-color': '#4f5b55', 'target-arrow-color': '#4f5b55' },
  },
  {
    selector: 'edge.analysis-highlight',
    style: {
      width: 5,
      'line-color': '#a83f31',
      'target-arrow-color': '#a83f31',
      opacity: 1,
      'z-index': 20,
    },
  },
]

function elements(): ElementDefinition[] {
  const visible = new Set(props.nodes.map((node) => node.id))
  return [
    ...props.nodes.map((node) => ({
      data: { id: node.id, label: node.title, kind: node.payload.kind },
    })),
    ...props.edges
      .filter((edge) => visible.has(edge.source) && visible.has(edge.destination))
      .map((edge) => ({
        data: {
          id: `${edge.source}:${edge.payload.kind}:${edge.destination}`,
          source: edge.source,
          target: edge.destination,
          kind: edge.payload.kind,
        },
      })),
  ]
}

function layout() {
  if (!graph || graph.nodes().length === 0) return
  graph.layout({
    name: 'breadthfirst',
    directed: true,
    padding: 48,
    spacingFactor: 1.35,
    animate: false,
  }).run()
  graph.fit(undefined, 48)
  if (graph.zoom() > 1.4) {
    graph.zoom(1.4)
    graph.center()
  }
}

function syncElements() {
  if (!graph) return
  graph.elements().remove()
  graph.add(elements())
  layout()
  syncSelection()
  syncHighlights()
}

function syncSelection() {
  if (!graph) return
  graph.nodes().unselect()
  if (props.selectedNodeId) graph.getElementById(props.selectedNodeId).select()
}

function syncHighlights() {
  if (!graph) return
  graph.elements().removeClass('analysis-highlight')
  for (const id of props.highlightedNodeIds ?? []) {
    graph.getElementById(id).addClass('analysis-highlight')
  }
  for (const id of props.highlightedEdgeIds ?? []) {
    graph.getElementById(id).addClass('analysis-highlight')
  }
}

function zoom(factor: number) {
  if (!graph) return
  graph.zoom({ level: graph.zoom() * factor, renderedPosition: { x: graph.width() / 2, y: graph.height() / 2 } })
}

function fit() {
  graph?.fit(undefined, 48)
}

onMounted(async () => {
  const { default: cytoscape } = await import('cytoscape')
  graph = cytoscape({
    container: container.value,
    elements: elements(),
    style: styles,
    minZoom: 0.35,
    maxZoom: 2.4,
    selectionType: 'single',
  })
  graph.on('tap', 'node', (event) => emit('select', event.target.id()))
  graph.on('tap', (event) => {
    if (event.target === graph) emit('select', null)
  })
  resizeObserver = new ResizeObserver(() => {
    if (resizeTimer) clearTimeout(resizeTimer)
    resizeTimer = setTimeout(() => graph?.resize(), 80)
  })
  if (container.value) resizeObserver.observe(container.value)
  layout()
  syncSelection()
  syncHighlights()
})

watch(graphSignature, syncElements)
watch(() => props.selectedNodeId, syncSelection)
watch(() => [props.highlightedNodeIds, props.highlightedEdgeIds], syncHighlights, { deep: true })

onBeforeUnmount(() => {
  resizeObserver?.disconnect()
  if (resizeTimer) clearTimeout(resizeTimer)
  graph?.destroy()
})
</script>

<template>
  <div class="graph-surface" data-testid="graph-surface">
    <div ref="container" class="graph-canvas" aria-label="System graph"></div>
    <p v-if="highlightedNodeIds?.length || highlightedEdgeIds?.length" class="sr-only" aria-live="polite">
      Analysis highlights {{ highlightedNodeIds?.length ?? 0 }} nodes and {{ highlightedEdgeIds?.length ?? 0 }} relationships.
    </p>
    <div class="zoom-controls" aria-label="Graph zoom controls">
      <button type="button" title="Zoom in" aria-label="Zoom in" @click="zoom(1.2)">
        <Plus :size="17" />
      </button>
      <button type="button" title="Zoom out" aria-label="Zoom out" @click="zoom(0.8)">
        <Minus :size="17" />
      </button>
      <button type="button" title="Fit graph" aria-label="Fit graph" @click="fit">
        <Focus :size="17" />
      </button>
    </div>
  </div>
</template>
