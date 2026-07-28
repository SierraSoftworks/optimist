<script setup lang="ts">
import cytoscape, { type Core, type ElementDefinition } from 'cytoscape'
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'

import type { Catalogue, SystemModel } from '../api/types'

const props = defineProps<{
  model: SystemModel
  catalogue?: Catalogue
  selected: string | null
  /** Constraint pressure per component, worst first, for colouring the graph. */
  pressure?: Record<string, number>
}>()

const emit = defineEmits<{
  select: [{ kind: 'component' | 'relationship'; id: string } | null]
  connect: [{ from: string; to: string }]
  move: [{ id: string; x: number; y: number }]
}>()

const host = ref<HTMLElement | null>(null)
let graph: Core | null = null
let observer: ResizeObserver | null = null

/**
 * Which side of the design a component sits on.
 *
 * Demand enters at components with no inbound relationships and leaves at those
 * with no outbound ones. Laying the graph out along that axis means a reader
 * follows a request down the page, which is the direction they already think in.
 */
function rank(model: SystemModel, id: string): number {
  const inbound = model.relationships.filter((edge) => edge.to === id).length
  const outbound = model.relationships.filter((edge) => edge.from === id).length
  if (inbound === 0) return 0
  if (outbound === 0) return 2
  return 1
}

function elements(): ElementDefinition[] {
  const nodes = props.model.components.map((component) => ({
    data: {
      id: component.id,
      label: component.name || component.id,
      kind: component.type,
      rank: rank(props.model, component.id),
      // Pressure drives colour rather than a separate badge, so a saturated
      // component is visible without reading anything.
      pressure: Math.min(props.pressure?.[component.id] ?? 0, 1.5),
      strained: (props.pressure?.[component.id] ?? 0) >= 1 ? 'yes' : 'no',
    },
    position: component.position ? { ...component.position } : undefined,
  }))
  const edges = props.model.relationships.map((edge) => ({
    data: {
      id: `${edge.from}\u2192${edge.to}`,
      source: edge.from,
      target: edge.to,
      label: edge.mutators.map((mutator) => mutator.type).join(', '),
    },
  }))
  return [...nodes, ...edges]
}

const STYLE: cytoscape.StylesheetJson = [
  {
    selector: 'node',
    style: {
      label: 'data(label)',
      'text-valign': 'center',
      'text-halign': 'center',
      'font-family': 'Manrope, sans-serif',
      'font-size': 12,
      'font-weight': 700,
      color: '#25292b',
      'text-wrap': 'wrap',
      'text-max-width': '120px',
      shape: 'round-rectangle',
      width: 140,
      height: 46,
      'background-color': '#f9faf7',
      'border-width': 1.5,
      'border-color': '#d6dad3',
    },
  },
  {
    selector: 'node[strained = "yes"]',
    style: { 'border-color': '#9a3e31', 'background-color': '#fff8f6', color: '#9a3e31' },
  },
  {
    selector: 'node:selected',
    style: { 'border-color': '#245746', 'border-width': 2.5, 'background-color': '#dce9e1' },
  },
  {
    selector: 'edge',
    style: {
      label: 'data(label)',
      'font-family': 'IBM Plex Mono, monospace',
      'font-size': 10,
      color: '#69716d',
      'text-background-color': '#eef0eb',
      'text-background-opacity': 1,
      'text-background-padding': '2px',
      width: 1.5,
      'line-color': '#b9c0b8',
      'target-arrow-color': '#b9c0b8',
      'target-arrow-shape': 'triangle',
      'curve-style': 'bezier',
      'arrow-scale': 1.1,
    },
  },
  {
    selector: 'edge:selected',
    style: { 'line-color': '#245746', 'target-arrow-color': '#245746', width: 2.5 },
  },
]

/**
 * Arranges whatever has not been arranged by hand.
 *
 * A design somebody has laid out is left exactly as they left it, because the
 * arrangement carries meaning no algorithm reconstructs. Automatic layout runs
 * only over components with no position of their own, which is why adding a
 * component to an arranged diagram places the newcomer without disturbing
 * anything already placed.
 *
 * Saved positions are reapplied before anything else, so returning to the
 * diagram redraws it as it was left rather than as an algorithm would have it.
 * Rebuilding the elements is not enough on its own: a node added without a
 * position keeps whatever the previous element in that slot had, which is how a
 * hand-made arrangement silently becomes a generated one.
 */
function layout(force = false) {
  if (!graph) return
  const saved = new Map(
    props.model.components
      .filter((component) => component.position)
      .map((component) => [component.id, component.position!] as const),
  )

  if (!force) {
    graph.nodes().forEach((node) => {
      const at = saved.get(node.id())
      if (at) node.position({ x: at.x, y: at.y })
    })
  }

  const arrange = force
    ? graph.nodes()
    : graph.nodes().filter((node) => !saved.has(node.id()))
  if (arrange.length === 0) {
    graph.fit(undefined, 45)
    return
  }

  const sources = arrange
    .filter((node) => node.data('rank') === 0)
    .map((node) => `#${node.id()}`)
  arrange
    .layout({
      name: 'breadthfirst',
      directed: true,
      spacingFactor: 1.25,
      padding: 40,
      animate: false,
      nodeDimensionsIncludeLabels: true,
      // Sources first, so the layout agrees with the rank above rather than
      // picking whichever node happened to be added first.
      roots: sources.length ? sources : undefined,
    })
    .run()
  graph.fit(undefined, 45)
}

onMounted(() => {
  if (!host.value) return
  graph = cytoscape({
    container: host.value,
    elements: elements(),
    style: STYLE,
    maxZoom: 2.5,
    minZoom: 0.2,
  })
  graph.on('tap', 'node', (event) =>
    emit('select', { kind: 'component', id: event.target.id() as string }),
  )
  graph.on('tap', 'edge', (event) =>
    emit('select', { kind: 'relationship', id: event.target.id() as string }),
  )
  graph.on('tap', (event) => {
    if (event.target === graph) emit('select', null)
  })
  // Reported when the drag ends rather than while it moves, so one placement is
  // one edit instead of a hundred.
  graph.on('dragfree', 'node', (event) => {
    const node = event.target
    const at = node.position()
    emit('move', { id: node.id() as string, x: Math.round(at.x), y: Math.round(at.y) })
  })
  // The container is measured by the layout, so it has to have been laid out
  // itself first. Running immediately puts every node in one row.
  requestAnimationFrame(() => layout())

  observer = new ResizeObserver(() => graph?.resize())
  observer.observe(host.value)
})

onBeforeUnmount(() => {
  observer?.disconnect()
  observer = null
  graph?.destroy()
  graph = null
})

/**
 * Rebuild only when the shape changes.
 *
 * Re-running the layout on every solved result would move the diagram under the
 * cursor each time a number changed, so the signature deliberately excludes
 * anything but the structure.
 */
const signature = () =>
  JSON.stringify([
    props.model.components.map((component) => [component.id, component.name, component.type]),
    props.model.relationships.map((edge) => [edge.from, edge.to, edge.mutators.length]),
  ])

watch(signature, () => {
  if (!graph) return
  graph.elements().remove()
  graph.add(elements())
  layout()
})

/**
 * Reapplies saved positions without rearranging anything.
 *
 * The signature above deliberately ignores positions, so a diagram drawn before
 * its positions arrived would otherwise keep the arrangement it was given at
 * mount and never adopt the one on record. Dragging is excluded by comparing
 * against where each node already is, so this cannot fight the pointer.
 */
watch(
  () => props.model.components.map((component) => [component.id, component.position] as const),
  (placements) => {
    if (!graph) return
    placements.forEach(([id, at]) => {
      if (!at) return
      const node = graph!.getElementById(id)
      if (node.empty()) return
      const now = node.position()
      if (Math.round(now.x) === at.x && Math.round(now.y) === at.y) return
      node.position({ x: at.x, y: at.y })
    })
  },
  { deep: true },
)
// Pressure is pushed into existing nodes rather than rebuilding, so colours
// update without disturbing positions.
watch(
  () => props.pressure,
  (pressure) => {
    graph?.nodes().forEach((node) => {
      const value = pressure?.[node.id()] ?? 0
      node.data('strained', value >= 1 ? 'yes' : 'no')
    })
  },
  { deep: true },
)

watch(
  () => props.selected,
  (id) => {
    if (!graph) return
    graph.elements().unselect()
    if (id) graph.getElementById(id).select()
  },
)

defineExpose({
  fit: () => graph?.fit(undefined, 45),
  relayout: () => layout(true),
})
</script>

<template>
  <div ref="host" class="graph" role="application" aria-label="System diagram" />
</template>

<style scoped>
.graph { width: 100%; height: 100%; background: var(--bg); }
</style>
