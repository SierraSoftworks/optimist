<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { Core, CytoscapeOptions, ElementDefinition } from 'cytoscape'
import { Focus, Minus, Pencil, Plus } from '@lucide/vue'
import type { GraphEdge, GraphNode } from '../api/types'
import { simulationReadiness } from '../domain/simulationReadiness'
import { edgeDisplayLabel } from '../domain/edgePresentation'

const props = defineProps<{
  nodes: GraphNode[]
  edges: GraphEdge[]
  selectedNodeId: string | null
  highlightedNodeIds?: string[]
  highlightedEdgeIds?: string[]
}>()

const emit = defineEmits<{
  select: [id: string | null]
  editEdge: [id: string]
  nodeContextmenu: [event: { nodeId: string; x: number; y: number }]
}>()
const container = ref<HTMLDivElement>()
let graph: Core | null = null
let resizeObserver: ResizeObserver | null = null
let resizeTimer: ReturnType<typeof setTimeout> | null = null

const graphSignature = computed(() =>
  JSON.stringify({
    nodes: props.nodes.map((node) => [
      node.id,
      node.title,
      node.payload.kind,
      simulationReadiness(node).level,
    ]),
    edges: props.edges.map((edge) => [
      edge.source,
      edge.payload.kind,
      edge.destination,
      edgeDisplayLabel(edge),
    ]),
  }),
)
const focusedEdges = computed(() =>
  props.selectedNodeId
    ? props.edges.filter(
        (edge) => edge.source === props.selectedNodeId || edge.destination === props.selectedNodeId,
      )
    : [],
)

function edgeId(edge: GraphEdge) {
  return `${edge.source}:${edge.payload.kind}:${edge.destination}`
}

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
    selector: 'node[readiness = "required"]',
    style: {
      'border-color': '#a83f31',
      'border-style': 'dashed',
      'border-width': 4,
    },
  },
  {
    selector: 'node[readiness = "recommended"]',
    style: {
      'border-color': '#9a6a12',
      'border-style': 'dotted',
      'border-width': 3,
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
      'overlay-opacity': 0,
      'overlay-padding': 10,
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
  {
    selector: 'edge.incident-edge',
    style: {
      width: 4,
      'line-color': '#245746',
      'target-arrow-color': '#245746',
      opacity: 1,
      label: 'data(detailLabel)',
      color: '#183f33',
      'font-family': 'IBM Plex Mono, monospace',
      'font-size': 9,
      'font-weight': 600,
      'text-background-color': '#ffffff',
      'text-background-opacity': 0.94,
      'text-background-padding': '4px',
      'text-border-color': '#b9c7bf',
      'text-border-width': 1,
      'text-border-opacity': 1,
      'text-rotation': 'none',
      'text-margin-y': -11,
      'z-index': 16,
    },
  },
  {
    selector: 'node.connected-node',
    style: {
      'underlay-color': '#4a8a70',
      'underlay-opacity': 0.09,
      'underlay-padding': 7,
    },
  },
]

function elements(): ElementDefinition[] {
  const visible = new Set(props.nodes.map((node) => node.id))
  return [
    ...props.nodes.map((node) => ({
      data: {
        id: node.id,
        label: node.title,
        kind: node.payload.kind,
        readiness: simulationReadiness(node).level,
      },
    })),
    ...props.edges
      .filter((edge) => visible.has(edge.source) && visible.has(edge.destination))
      .map((edge) => ({
        data: {
          id: edgeId(edge),
          source: edge.source,
          target: edge.destination,
          kind: edge.payload.kind,
          detailLabel: edgeDisplayLabel(edge),
        },
      })),
  ]
}

function layout() {
  if (!graph || graph.nodes().length === 0) return
  const interventions = graph.nodes('[kind = "intervention"]')
  graph.layout({
    name: 'breadthfirst',
    directed: true,
    roots: interventions.length ? interventions.map((node) => node.id()) : undefined,
    circle: false,
    padding: 48,
    spacingFactor: 1.5,
    animate: false,
  }).run()
  enforceKindBands()
  graph.fit(undefined, 48)
  if (graph.zoom() > 1.4) {
    graph.zoom(1.4)
    graph.center()
  }
}

function enforceKindBands() {
  if (!graph || graph.nodes().length < 2) return
  const positions = graph.nodes().map((node) => node.position('y'))
  const top = Math.min(...positions)
  const bottom = Math.max(Math.max(...positions), top + 220)
  const middleTop = top + 82
  const middleBottom = bottom - 82
  graph.nodes('[kind = "intervention"]').forEach((node) => {
    node.position('y', top)
  })
  graph.nodes('[kind = "outcome"]').forEach((node) => {
    node.position('y', bottom)
  })
  graph.nodes().forEach((node) => {
    if (node.data('kind') === 'intervention' || node.data('kind') === 'outcome') return
    node.position('y', Math.min(middleBottom, Math.max(middleTop, node.position('y'))))
  })
}

function syncElements() {
  if (!graph) return
  graph.elements().remove()
  graph.add(elements())
  layout()
  syncSelection()
  syncFocus()
  syncHighlights()
}

function syncSelection() {
  if (!graph) return
  graph.nodes().unselect()
  if (props.selectedNodeId) graph.getElementById(props.selectedNodeId).select()
}

function syncFocus() {
  if (!graph) return
  graph.elements().removeClass('incident-edge connected-node')
  if (!props.selectedNodeId) return
  const selected = graph.getElementById(props.selectedNodeId)
  selected.connectedEdges().addClass('incident-edge')
  selected.neighborhood('node').addClass('connected-node')
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
  graph.on('tap', 'edge', (event) => emit('editEdge', event.target.id()))
  graph.on('cxttap', 'node', (event) => {
    const bounds = container.value?.getBoundingClientRect()
    if (!bounds) return
    emit('nodeContextmenu', {
      nodeId: event.target.id(),
      x: bounds.left + event.renderedPosition.x,
      y: bounds.top + event.renderedPosition.y,
    })
  })
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
  syncFocus()
  syncHighlights()
})

watch(graphSignature, syncElements)
watch(() => props.selectedNodeId, () => {
  syncSelection()
  syncFocus()
})
watch(() => [props.highlightedNodeIds, props.highlightedEdgeIds], syncHighlights, { deep: true })

onBeforeUnmount(() => {
  resizeObserver?.disconnect()
  if (resizeTimer) clearTimeout(resizeTimer)
  graph?.destroy()
})
</script>

<template>
  <div class="graph-surface" data-testid="graph-surface">
    <div ref="container" class="graph-canvas" aria-label="System graph" @contextmenu.prevent></div>
    <p v-if="highlightedNodeIds?.length || highlightedEdgeIds?.length" class="sr-only" aria-live="polite">
      Analysis highlights {{ highlightedNodeIds?.length ?? 0 }} nodes and {{ highlightedEdgeIds?.length ?? 0 }} relationships.
    </p>
    <section v-if="focusedEdges.length" class="focused-relationships" aria-label="Focused relationships">
      <header><strong>Focused relationships</strong><span>{{ focusedEdges.length }}</span></header>
      <button
        v-for="edge in focusedEdges"
        :key="edgeId(edge)"
        type="button"
        :aria-label="`Edit focused relationship ${edge.source} ${edge.payload.kind.replaceAll('_', ' ')} ${edge.destination}`"
        @click="emit('editEdge', edgeId(edge))"
      >
        <span><code>{{ edge.source }} → {{ edge.destination }}</code><small>{{ edgeDisplayLabel(edge) }}</small></span>
        <Pencil :size="13" />
      </button>
    </section>
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

<style scoped>
.graph-surface, .graph-canvas { position: absolute; inset: 0; }
.focused-relationships { position: absolute; z-index: 3; left: 14px; bottom: 14px; width: min(300px, calc(100% - 86px)); max-height: 168px; overflow: auto; border: 1px solid #aeb9b1; border-radius: 6px; background: rgba(255,255,255,.96); box-shadow: 0 8px 22px rgba(30,40,34,.12); }
.focused-relationships header { position: sticky; top: 0; display: flex; justify-content: space-between; gap: 8px; padding: 7px 9px; border-bottom: 1px solid var(--line); background: #f7f9f5; }
.focused-relationships header strong { font-size: 9px; text-transform: uppercase; letter-spacing: .06em; }
.focused-relationships header span { color: var(--muted); font: 9px 'IBM Plex Mono', monospace; }
.focused-relationships button { width: 100%; min-height: 42px; display: grid; grid-template-columns: minmax(0, 1fr) 20px; gap: 8px; align-items: center; padding: 6px 8px; border: 0; border-bottom: 1px solid #e7eae5; background: transparent; color: var(--ink); text-align: left; }
.focused-relationships button:last-child { border-bottom: 0; }
.focused-relationships button:hover, .focused-relationships button:focus-visible { background: var(--green-soft); }
.focused-relationships button > span { min-width: 0; display: grid; gap: 2px; }
.focused-relationships code, .focused-relationships small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.focused-relationships code { font: 9px 'IBM Plex Mono', monospace; }
.focused-relationships small { color: var(--green); font-size: 8px; font-weight: 650; }
.zoom-controls { position: absolute; right: 14px; bottom: 14px; display: grid; gap: 4px; padding: 4px; border: 1px solid var(--line); border-radius: 6px; background: white; }
.zoom-controls button { width: 30px; height: 30px; display: grid; place-items: center; border: 0; border-radius: 4px; background: transparent; color: var(--muted); }
.zoom-controls button:hover { background: #edf0eb; color: var(--ink); }

@media (max-width: 760px) {
  .focused-relationships { max-height: 128px; }
}
</style>
