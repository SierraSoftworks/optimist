<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { Core, CytoscapeOptions, ElementDefinition } from 'cytoscape'
import { Focus, GitBranch, LayoutGrid, Minus, Pencil, Plus } from '@lucide/vue'
import type { GraphEdge, GraphNode } from '../api/types'
import { simulationReadiness } from '../domain/simulationReadiness'
import { edgeDisplayLabel } from '../domain/edgePresentation'
import { forceLayout } from '../domain/graphLayout'
import {
  clusteredPositions,
  defaultGraphLayout,
  graphDetailForZoom,
  type GraphDetail,
  type GraphLayoutMode,
} from '../domain/graphView'

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
const detail = ref<GraphDetail>('detail')
const layoutMode = ref<GraphLayoutMode>(defaultGraphLayout(props.nodes.length))
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
const kindClusters = computed(() => [
  { kind: 'intervention', label: 'Actions' },
  { kind: 'factor', label: 'Factors' },
  { kind: 'metric', label: 'Metrics' },
  { kind: 'outcome', label: 'Objectives' },
].map((cluster) => ({
  ...cluster,
  count: props.nodes.filter((node) => node.payload.kind === cluster.kind).length,
})).filter((cluster) => cluster.count > 0))

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
    selector: 'node.semantic-context',
    style: {
      'font-size': 9,
      'text-max-width': '82px',
      'text-margin-y': 7,
    },
  },
  {
    selector: 'edge.semantic-context',
    style: { opacity: 0.42, width: 1 },
  },
  {
    selector: 'node.semantic-overview',
    style: { width: 28, height: 28, label: '' },
  },
  {
    selector: 'edge.semantic-overview',
    style: {
      opacity: 0.2,
      width: 0.8,
      'target-arrow-shape': 'none',
    },
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
  {
    selector: 'node.semantic-overview:selected, node.semantic-overview.connected-node, node.semantic-overview.analysis-highlight',
    style: {
      width: 46,
      height: 46,
      label: 'data(label)',
      'font-size': 10,
      'text-max-width': '100px',
    },
  },
  {
    selector: 'edge.semantic-overview.incident-edge, edge.semantic-overview.analysis-highlight',
    style: {
      opacity: 1,
      'target-arrow-shape': 'triangle',
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
  if (layoutMode.value === 'clusters') {
    const positions = Object.fromEntries(clusteredPositions(props.nodes))
    graph.layout({
      name: 'preset',
      positions,
      padding: 64,
      animate: false,
    }).run()
  } else {
    const positions = Object.fromEntries(forceLayout(
      props.nodes.map((node) => ({ id: node.id, kind: node.payload.kind })),
      props.edges
        .filter((edge) => edge.payload.kind !== 'measures')
        .map((edge) => ({ source: edge.source, destination: edge.destination })),
    ))
    graph.layout({
      name: 'preset',
      positions,
      padding: 48,
      animate: false,
    }).run()
  }
  graph.fit(undefined, 48)
  if (graph.zoom() > 1.4) {
    graph.zoom(1.4)
    graph.center()
  }
  syncSemanticZoom()
}

function setLayoutMode(mode: GraphLayoutMode) {
  if (layoutMode.value === mode) return
  layoutMode.value = mode
  layout()
  syncSelection()
  syncFocus()
  syncHighlights()
}

function syncElements() {
  if (!graph) return
  graph.elements().remove()
  graph.add(elements())
  layout()
  syncSelection()
  syncFocus()
  syncHighlights()
  syncSemanticZoom()
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
  syncSemanticZoom()
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
  syncSemanticZoom()
}

function syncSemanticZoom() {
  if (!graph) return
  detail.value = graphDetailForZoom(graph.zoom())
  graph.elements().removeClass('semantic-overview semantic-context')
  if (detail.value === 'overview') graph.elements().addClass('semantic-overview')
  if (detail.value === 'context') graph.elements().addClass('semantic-context')
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
  if (!container.value) return
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
  graph.on('zoom', syncSemanticZoom)
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
    <div class="graph-view-controls">
      <div class="layout-switch" role="group" aria-label="Graph layout">
        <button type="button" title="Hierarchy layout" aria-label="Hierarchy layout" :aria-pressed="layoutMode === 'hierarchy'" @click="setLayoutMode('hierarchy')"><GitBranch :size="14" /></button>
        <button type="button" title="Cluster by kind" aria-label="Cluster by kind" :aria-pressed="layoutMode === 'clusters'" @click="setLayoutMode('clusters')"><LayoutGrid :size="14" /></button>
      </div>
      <span class="detail-indicator" :data-detail="detail" aria-live="polite">{{ detail }}</span>
    </div>
    <div v-if="layoutMode === 'clusters'" class="cluster-legend" aria-label="Node kind clusters">
      <span v-for="cluster in kindClusters" :key="cluster.kind"><i :data-kind="cluster.kind"></i>{{ cluster.label }} <strong>{{ cluster.count }}</strong></span>
    </div>
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
.graph-view-controls { position: absolute; z-index: 4; top: 12px; right: 14px; display: flex; align-items: center; gap: 6px; }
.layout-switch { display: flex; padding: 3px; border: 1px solid var(--line); border-radius: 6px; background: white; }
.layout-switch button { width: 28px; height: 26px; display: grid; place-items: center; padding: 0; border: 0; border-radius: 4px; background: transparent; color: var(--muted); }
.layout-switch button:hover { color: var(--ink); }
.layout-switch button[aria-pressed='true'] { background: var(--green-soft); color: var(--green); }
.detail-indicator { min-width: 58px; padding: 5px 7px; border: 1px solid var(--line); border-radius: 5px; background: rgba(255,255,255,.94); color: var(--muted); font-size: var(--text-2xs); font-weight: 700; text-align: center; text-transform: uppercase; }
.detail-indicator[data-detail='overview'] { border-color: #d4b171; color: #795710; }
.cluster-legend { position: absolute; z-index: 3; top: 50px; right: 14px; display: grid; gap: 4px; padding: 7px 8px; border: 1px solid var(--line); border-radius: 6px; background: rgba(255,255,255,.94); }
.cluster-legend span { display: grid; grid-template-columns: 10px minmax(58px, 1fr) auto; gap: 5px; align-items: center; color: var(--muted); font-size: var(--text-2xs); }
.cluster-legend i { width: 8px; height: 8px; border-radius: 2px; }
.cluster-legend strong { color: var(--ink); font: var(--text-2xs) var(--mono); }
.focused-relationships { position: absolute; z-index: 3; left: 14px; bottom: 14px; width: min(300px, calc(100% - 86px)); max-height: 168px; overflow: auto; border: 1px solid #aeb9b1; border-radius: 6px; background: rgba(255,255,255,.96); box-shadow: 0 8px 22px rgba(30,40,34,.12); }
.focused-relationships header { position: sticky; top: 0; display: flex; justify-content: space-between; gap: 8px; padding: 7px 9px; border-bottom: 1px solid var(--line); background: #f7f9f5; }
.focused-relationships header strong { font-size: var(--text-xs); text-transform: uppercase; letter-spacing: .06em; }
.focused-relationships header span { color: var(--muted); font: var(--text-xs) var(--mono); }
.focused-relationships button { width: 100%; min-height: 42px; display: grid; grid-template-columns: minmax(0, 1fr) 20px; gap: 8px; align-items: center; padding: 6px 8px; border: 0; border-bottom: 1px solid #e7eae5; background: transparent; color: var(--ink); text-align: left; }
.focused-relationships button:last-child { border-bottom: 0; }
.focused-relationships button:hover, .focused-relationships button:focus-visible { background: var(--green-soft); }
.focused-relationships button > span { min-width: 0; display: grid; gap: 2px; }
.focused-relationships code, .focused-relationships small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.focused-relationships code { font: var(--text-xs) var(--mono); }
.focused-relationships small { color: var(--green); font-size: var(--text-2xs); font-weight: 650; }
.zoom-controls { position: absolute; right: 14px; bottom: 14px; display: grid; gap: 4px; padding: 4px; border: 1px solid var(--line); border-radius: 6px; background: white; }
.zoom-controls button { width: 30px; height: 30px; display: grid; place-items: center; border: 0; border-radius: 4px; background: transparent; color: var(--muted); }
.zoom-controls button:hover { background: #edf0eb; color: var(--ink); }

@media (max-width: 760px) {
  .graph-view-controls { top: 48px; }
  .cluster-legend { top: 86px; }
  .focused-relationships { max-height: 128px; }
}
</style>
