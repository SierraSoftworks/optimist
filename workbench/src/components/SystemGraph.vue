<script setup lang="ts">
import cytoscape, { type Core, type ElementDefinition } from 'cytoscape'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import type { Bottleneck, Catalogue, SystemModel } from '../api/types'
import { glyphFor, glyphUri } from '../domain/componentIcons'
import { formatSiNumber } from '../domain/humanNumber'
import { inhabited, owner } from '../domain/scaleUnits'

const props = defineProps<{
  model: SystemModel
  catalogue?: Catalogue
  selected: string | null
  /**
   * What each component is closest to exhausting, worst first.
   *
   * Drives the colour and the flyout together, so what a reader is told when
   * they stop on a red component is the reason it is red rather than a second
   * measurement that might disagree with it.
   */
  constraints?: Record<string, Bottleneck[]>
}>()

const emit = defineEmits<{
  select: [{ kind: 'component' | 'relationship'; id: string } | null]
  connect: [{ from: string; to: string }]
  move: [{ id: string; x: number; y: number }]
  create: [{ type: string; x: number; y: number }]
  remove: [{ id: string }]
}>()

const host = ref<HTMLElement | null>(null)
let graph: Core | null = null
let observer: ResizeObserver | null = null

/** How loaded each component's worst constraint is. */
const pressure = computed(() => {
  const worst: Record<string, number> = {}
  for (const [component, entries] of Object.entries(props.constraints ?? {})) {
    worst[component] = Math.max(0, ...entries.map((entry) => entry.utilisation))
  }
  return worst
})

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

/**
 * The diagram node standing for a scale unit.
 *
 * Namespaced away from component identifiers, which share the same space in
 * Cytoscape but not in the model.
 */
function boundary(unit: string): string {
  return `unit:${unit}`
}

/**
 * The scale units worth drawing, which are the ones containing something.
 *
 * An empty unit has no boundary to draw. Drawing one anyway would put an empty
 * box on the diagram that nothing is inside and that dragging cannot fix, since
 * membership is decided in the panel rather than on the canvas.
 */
function boundaries() {
  const drawn = inhabited(props.model.scale_units)
  return props.model.scale_units
    .filter((unit) => drawn.has(unit.id))
    .map((unit) => ({
      data: {
        id: boundary(unit.id),
        label: `${unit.name || unit.id} \u00d7${unit.replicas.trim() || '1'}`,
        unit: 'yes',
        mirrored: unit.distribution === 'mirrored' ? 'yes' : 'no',
        parent: unit.parent && drawn.has(unit.parent) ? boundary(unit.parent) : undefined,
      },
    }))
}

function elements(): ElementDefinition[] {
  const nodes = props.model.components.map((component) => {
    const strained = (pressure.value[component.id] ?? 0) >= 1
    const holder = owner(props.model.scale_units, component.id)
    return {
      data: {
        id: component.id,
        label: component.name || component.id,
        kind: component.type,
        rank: rank(props.model, component.id),
        // Membership is expressed as containment, which is what a scale unit
        // means. Nesting comes free with it: a unit inside another is drawn
        // inside it because it says so, without the diagram knowing about
        // chains at all.
        parent: holder ? boundary(holder.id) : undefined,
        // The type's own glyph, drawn into the node rather than beside it. A
        // diagram is scanned for shape before it is read for words, and a row of
        // identical rectangles makes a reader read every label to find the store.
        glyph: glyphUri(
          props.catalogue?.component_types[component.type]?.icon,
          strained ? '#9a3e31' : '#69716d',
        ),
        strained: strained ? 'yes' : 'no',
      },
      position: component.position ? { ...component.position } : undefined,
    }
  })
  const edges = props.model.relationships.map((edge) => ({
    data: {
      id: `${edge.from}\u2192${edge.to}`,
      source: edge.from,
      target: edge.to,
      label: edge.mutators.map((mutator) => mutator.type).join(', '),
    },
  }))
  // Boundaries first: Cytoscape needs a compound parent to exist before the
  // element naming it as their parent is added.
  return [...boundaries(), ...nodes, ...edges]
}

const STYLE: cytoscape.StylesheetJson = [
  {
    selector: 'node',
    style: {
      label: 'data(label)',
      'text-valign': 'center',
      'text-halign': 'center',
      'text-margin-x': 11,
      'font-family': 'Montserrat Variable, sans-serif',
      'font-size': 12,
      'font-weight': 700,
      color: '#25292b',
      'text-wrap': 'wrap',
      'text-max-width': '96px',
      shape: 'round-rectangle',
      width: 148,
      height: 46,
      'background-color': '#f9faf7',
      // The image is composed to the node's proportions, so fitting it inside
      // puts the glyph exactly where the image says it goes.
      'background-image': 'data(glyph)',
      'background-fit': 'contain',
      'background-clip': 'node',
      'background-image-opacity': 1,
      'border-width': 1.5,
      'border-color': '#d6dad3',
    },
  },
  {
    selector: 'node[strained = "yes"]',
    style: { 'border-color': '#9a3e31', 'background-color': '#fff8f6', color: '#9a3e31' },
  },
  /*
   * A scale unit is a boundary rather than a thing, so it is drawn as one: a
   * dashed enclosure labelled with what it is and how many of it there are.
   * Nothing is edited by pointing at it — membership is a decision about the
   * model, not a position — so it takes no events at all and a click on the
   * space inside it reaches the canvas underneath.
   */
  {
    selector: 'node[unit = "yes"]',
    style: {
      label: 'data(label)',
      shape: 'round-rectangle',
      padding: '22px',
      // Hung outside the top-left corner. Traffic runs down the diagram and
      // therefore crosses the top edge of every boundary it enters, and edge
      // labels are drawn over nodes, so a title centred there is overwritten by
      // whichever behaviour happens to sit on the wire.
      'text-valign': 'top',
      'text-halign': 'left',
      'text-margin-x': -6,
      'text-margin-y': -4,
      'text-max-width': '240px',
      'font-family': 'Fira Code Variable, monospace',
      'font-size': 10,
      'font-weight': 600,
      color: '#69716d',
      // The label sits on the boundary's own edge, which crosses whatever the
      // enclosing unit holds. Without a background it reads as two overlapping
      // words rather than as either of them.
      'text-background-color': '#eef0eb',
      'text-background-opacity': 1,
      'text-background-padding': '3px',      'background-color': '#e3e7de',
      'background-opacity': 0.55,
      'background-image': 'none',
      'border-width': 1.5,
      'border-style': 'dashed',
      'border-color': '#b9c0b8',
      events: 'no',
    },
  },
  // Mirrored units multiply cost without dividing load, which is the surprising
  // one, so it is the one the diagram distinguishes.
  {
    selector: 'node[unit = "yes"][mirrored = "yes"]',
    style: { 'border-style': 'dotted', 'border-color': '#a08a5c' },
  },
  {
    selector: 'node:selected',
    style: { 'border-color': '#245746', 'border-width': 2.5, 'background-color': '#dce9e1' },
  },
  {
    selector: 'edge',
    style: {
      label: 'data(label)',
      'font-family': 'Fira Code Variable, monospace',
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

  // A boundary has no position of its own: it is drawn around whatever is
  // inside it, so laying it out would fight its own children.
  const placeable = graph.nodes().filter((node) => node.data('unit') !== 'yes')
  const arrange = force ? placeable : placeable.filter((node) => !saved.has(node.id()))
  if (arrange.length === 0) {
    reframe()
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
  reframe()
}

/**
 * Frames the whole diagram once it has settled to its final size.
 *
 * A boundary is sized by what it contains, and that size is only known after a
 * render. Fitting in the same tick as the layout therefore frames the diagram as
 * it was before the boundaries grew, which leaves the bottom of a nested design
 * off-screen.
 */
function reframe() {
  requestAnimationFrame(() => graph?.fit(undefined, 45))
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

  // Stopping on a component says why it is the colour it is. Reported in
  // rendered coordinates because the flyout is HTML over the canvas, and the
  // canvas has its own pan and zoom.
  graph.on('mouseover', 'node', (event) => {
    const at = event.target.renderedPosition()
    const box = event.target.renderedBoundingBox()
    hovered.value = { id: event.target.id() as string, x: box.x2, y: at.y }
  })
  graph.on('mouseout', 'node', () => (hovered.value = null))
  graph.on('pan zoom drag', () => (hovered.value = null))

  graph.on('cxttap', (event) => {
    const rendered = event.renderedPosition ?? { x: 0, y: 0 }
    hovered.value = null
    if (event.target === graph) {
      // Both positions are kept: one to put the menu where the pointer is, one
      // to put the component where the diagram was clicked.
      menu.value = {
        kind: 'canvas',
        x: rendered.x,
        y: rendered.y,
        at: event.position ?? { x: 0, y: 0 },
      }
      return
    }
    if (event.target.isNode?.()) {
      menu.value = { kind: 'component', x: rendered.x, y: rendered.y, id: event.target.id() }
    }
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
  requestAnimationFrame(() => {
    layout()
    warmGlyphs()
  })

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
 *
 * A component's glyph is part of that structure even though it comes from the
 * catalogue rather than the model: the catalogue arrives after the design does,
 * and without it here every node would keep the anonymous box it was drawn with
 * before anything knew what it was.
 */
const signature = () =>
  JSON.stringify([
    props.model.components.map((component) => [
      component.id,
      component.name,
      component.type,
      props.catalogue?.component_types[component.type]?.icon,
    ]),
    props.model.relationships.map((edge) => [edge.from, edge.to, edge.mutators.length]),
    props.model.scale_units.map((unit) => [
      unit.id,
      unit.name,
      unit.replicas,
      unit.distribution,
      unit.parent,
      unit.members,
    ]),
  ])

watch(signature, () => {
  if (!graph) return
  graph.elements().remove()
  graph.add(elements())
  layout()
  warmGlyphs()
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
  pressure,
  (loads) => {
    graph?.nodes().forEach((node) => {
      if (node.data('unit') === 'yes') return
      const strained = (loads[node.id()] ?? 0) >= 1
      node.data('strained', strained ? 'yes' : 'no')
      const type = props.model.components.find((component) => component.id === node.id())?.type
      node.data(
        'glyph',
        glyphUri(
          type ? props.catalogue?.component_types[type]?.icon : undefined,
          strained ? '#9a3e31' : '#69716d',
        ),
      )
    })
    warmGlyphs()
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

/**
 * Draws again once every glyph has loaded.
 *
 * Cytoscape paints to a canvas as soon as it has elements, and an image that has
 * not finished decoding is simply absent from that paint. Nothing schedules
 * another one, so the glyphs stayed invisible until something unrelated — a
 * drag, a resize — happened to force a redraw. Waiting for the images and asking
 * for one is the whole fix.
 */
function warmGlyphs() {
  if (!graph) return
  const sources = new Set(
    graph
      .nodes()
      .map((node) => node.data('glyph') as string | undefined)
      .filter((source): source is string => !!source),
  )
  if (sources.size === 0) return
  let outstanding = sources.size
  const done = () => {
    outstanding -= 1
    if (outstanding === 0) graph?.forceRender()
  }
  sources.forEach((source) => {
    const image = new Image()
    image.onload = done
    image.onerror = done
    image.src = source
  })
}

/**
 * What a component is closest to exhausting, shown where the component is.
 *
 * A red box says something is wrong and not what. The constraints behind that
 * colour are what an author needs in order to do anything about it, and sending
 * them to the simulation to find out means leaving the diagram they were
 * reasoning about.
 */
const hovered = ref<{ id: string; x: number; y: number } | null>(null)

const hoveredConstraints = computed(() => {
  const entries = hovered.value ? (props.constraints?.[hovered.value.id] ?? []) : []
  return [...entries].sort((a, b) => b.utilisation - a.utilisation)
})

/** Right-click menus, which is how a diagram is edited without a toolbar. */
const menu = ref<
  | { kind: 'canvas'; x: number; y: number; at: { x: number; y: number } }
  | { kind: 'component'; x: number; y: number; id: string }
  | null
>(null)

const connectable = computed(() => {
  if (menu.value?.kind !== 'component') return []
  const from = menu.value.id
  const attached = new Set(
    props.model.relationships
      .filter((edge) => edge.from === from)
      .map((edge) => edge.to),
  )
  return props.model.components.filter(
    (component) => component.id !== from && !attached.has(component.id),
  )
})

const types = computed(() => Object.values(props.catalogue?.component_types ?? {}))

function closeMenu() {
  menu.value = null
}

function create(type: string) {
  if (menu.value?.kind !== 'canvas') return
  emit('create', { type, x: Math.round(menu.value.at.x), y: Math.round(menu.value.at.y) })
  closeMenu()
}

function connectTo(to: string) {
  if (menu.value?.kind !== 'component') return
  emit('connect', { from: menu.value.id, to })
  closeMenu()
}

function removeComponent() {
  if (menu.value?.kind !== 'component') return
  emit('remove', { id: menu.value.id })
  closeMenu()
}

function glyph(type: string) {
  return glyphFor(props.catalogue?.component_types[type]?.icon)
}

/** Load as a share of the limit, kept readable when a design is far past it. */
function load(utilisation: number): string {
  return utilisation >= 10
    ? `\u00d7${formatSiNumber(utilisation, 2)}`
    : `${(utilisation * 100).toFixed(0)}%`
}

defineExpose({
  fit: () => graph?.fit(undefined, 45),
  relayout: () => layout(true),
})
</script>

<template>
  <!--
    The browser's own context menu is suppressed over the diagram. Two menus
    appearing on one gesture is not a choice anybody wants to make, and the one
    with "Reload" in it is not the one being offered here.
  -->
  <div class="frame" @contextmenu.prevent @keydown.esc="closeMenu">
    <div ref="host" class="graph" role="application" aria-label="System diagram" />

    <div
      v-if="hovered && hoveredConstraints.length"
      class="flyout"
      :style="{ left: `${hovered.x}px`, top: `${hovered.y}px` }"
      data-test="component-limits"
    >
      <p class="heading">Closest to its limit</p>
      <div
        v-for="entry in hoveredConstraints"
        :key="entry.constraint"
        class="constraint"
        :class="{ binding: entry.probability_of_binding > 0 }"
      >
        <div class="row">
          <span class="name">{{ entry.constraint }}</span>
          <span class="load">{{ load(entry.utilisation) }}</span>
        </div>
        <div class="gauge"><span :style="{ width: `${Math.min(entry.utilisation, 1) * 100}%` }" /></div>
        <p class="says">{{ entry.summary }}</p>
      </div>
    </div>

    <!--
      A right-click menu rather than a toolbar dialog. Adding a component is a
      placement as much as a choice, and a dialog that appears in the middle of
      the screen loses the one piece of information the gesture carried.
    -->
    <template v-if="menu">
      <div class="scrim" @click="closeMenu" @contextmenu.prevent="closeMenu" />
      <div class="menu" :style="{ left: `${menu.x}px`, top: `${menu.y}px` }" data-test="graph-menu">
        <template v-if="menu.kind === 'canvas'">
          <p class="label">Add here</p>
          <button
            v-for="type in types"
            :key="type.id"
            class="item"
            :data-test="`place-${type.id}`"
            :title="type.summary"
            @click="create(type.id)"
          >
            <el-icon class="mark"><component :is="glyph(type.id)" /></el-icon>
            <span>{{ type.name }}</span>
          </button>
        </template>

        <template v-else>
          <p class="label">{{ menu.id }}</p>
          <div v-if="connectable.length" class="submenu">
            <button class="item" data-test="connect-to">
              <el-icon class="mark"><i-connection /></el-icon>
              <span>Connect to</span>
              <el-icon class="chevron"><i-right /></el-icon>
            </button>
            <div class="nested">
              <button
                v-for="component in connectable"
                :key="component.id"
                class="item"
                :data-test="`connect-to-${component.id}`"
                @click="connectTo(component.id)"
              >
                <el-icon class="mark"><component :is="glyph(component.type)" /></el-icon>
                <span>{{ component.name || component.id }}</span>
              </button>
            </div>
          </div>
          <button class="item danger" data-test="remove-component" @click="removeComponent">
            <el-icon class="mark"><i-delete /></el-icon>
            <span>Remove</span>
          </button>
        </template>
      </div>
    </template>
  </div>
</template>

<style scoped>
.frame { position: relative; width: 100%; height: 100%; }
.graph { width: 100%; height: 100%; background: var(--bg); }

.flyout {
  position: absolute;
  z-index: 3;
  width: 236px;
  transform: translate(14px, -50%);
  padding: var(--space-2) var(--space-3) var(--space-3);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface-strong);
  box-shadow: 0 8px 26px rgb(28 35 31 / 18%);
  pointer-events: none;
}
.heading {
  margin: 0 0 var(--space-2);
  font-family: var(--display);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--muted);
  font-weight: 700;
}
.constraint + .constraint { margin-top: var(--space-2); padding-top: var(--space-2); border-top: 1px solid var(--line); }
.row { display: flex; align-items: baseline; justify-content: space-between; gap: var(--space-2); }
.name { font-family: var(--mono); font-size: var(--text-2xs); }
.load { font-family: var(--mono); font-size: var(--text-sm); }
.constraint.binding .load, .constraint.binding .name { color: var(--danger); }
.gauge { height: 3px; margin: 3px 0; border-radius: 2px; background: var(--line); overflow: hidden; }
.gauge span { display: block; height: 100%; background: var(--green); }
.constraint.binding .gauge span { background: var(--danger); }
.says { margin: 0; font-size: 10px; line-height: 1.35; color: var(--muted); }

.scrim { position: fixed; inset: 0; z-index: 4; }
.menu {
  position: absolute;
  z-index: 5;
  min-width: 176px;
  padding: var(--space-1);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface-strong);
  box-shadow: 0 8px 26px rgb(28 35 31 / 18%);
}
.label {
  margin: 0;
  padding: var(--space-1) var(--space-2) var(--space-2);
  font-family: var(--display);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--muted);
  font-weight: 700;
}
.item {
  width: 100%;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: 5px var(--space-2);
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  text-align: left;
  font-size: var(--text-sm);
  color: var(--ink);
}
.item:hover { background: var(--green-soft); }
.item.danger { color: var(--danger); }
.item.danger:hover { background: var(--danger-surface); }
.mark { font-size: 13px; color: var(--muted); flex: 0 0 auto; }
.item.danger .mark { color: var(--danger); }
.chevron { margin-left: auto; font-size: 11px; color: var(--muted); }

.submenu { position: relative; }
.nested {
  display: none;
  position: absolute;
  left: 100%;
  top: 0;
  min-width: 168px;
  max-height: 280px;
  overflow: auto;
  padding: var(--space-1);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface-strong);
  box-shadow: 0 8px 26px rgb(28 35 31 / 18%);
}
.submenu:hover .nested { display: block; }
</style>
