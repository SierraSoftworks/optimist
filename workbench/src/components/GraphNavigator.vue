<script setup lang="ts">
import { nextTick, ref } from 'vue'
import { AlertTriangle, CircleDot, List, Table2 } from '@lucide/vue'
import type { GraphNode } from '../api/types'
import { readinessLabel, simulationReadiness } from '../domain/simulationReadiness'

const props = defineProps<{
  nodes: GraphNode[]
  selectedNodeId: string | null
}>()
const emit = defineEmits<{ select: [id: string] }>()
const view = ref<'outline' | 'table'>('outline')
const outlineButtons = ref<Array<HTMLButtonElement>>([])
const tableButtons = ref<Array<HTMLButtonElement>>([])

function select(node: GraphNode) {
  emit('select', node.id)
}

function buttonRefs() {
  return view.value === 'outline' ? outlineButtons.value : tableButtons.value
}

async function move(event: KeyboardEvent, index: number) {
  let next = index
  if (event.key === 'ArrowDown' || event.key === 'ArrowRight') next = Math.min(props.nodes.length - 1, index + 1)
  else if (event.key === 'ArrowUp' || event.key === 'ArrowLeft') next = Math.max(0, index - 1)
  else if (event.key === 'Home') next = 0
  else if (event.key === 'End') next = props.nodes.length - 1
  else return
  event.preventDefault()
  const node = props.nodes[next]
  if (!node) return
  emit('select', node.id)
  await nextTick()
  buttonRefs()[next]?.focus()
}

function tabIndex(node: GraphNode, index: number) {
  if (props.selectedNodeId) return props.selectedNodeId === node.id ? 0 : -1
  return index === 0 ? 0 : -1
}
</script>

<template>
  <section class="outline-section" aria-label="Graph accessibility view">
    <div class="section-title navigator-view-header">
      <span class="section-label">Nodes</span>
      <div class="navigator-view-tabs" role="group" aria-label="Node view">
        <button type="button" :aria-pressed="view === 'outline'" title="Outline view" aria-label="Outline view" @click="view = 'outline'"><List :size="13" /></button>
        <button type="button" :aria-pressed="view === 'table'" title="Table view" aria-label="Table view" @click="view = 'table'"><Table2 :size="13" /></button>
      </div>
      <span>{{ nodes.length }}</span>
    </div>

    <div v-if="view === 'outline'" class="node-outline" aria-label="Node outline">
      <button
        v-for="(node, index) in nodes"
        :key="node.id"
        :ref="(element) => { if (element) outlineButtons[index] = element as HTMLButtonElement }"
        type="button"
        :tabindex="tabIndex(node, index)"
        :class="{ selected: selectedNodeId === node.id }"
        :data-readiness="simulationReadiness(node).level"
        :title="readinessLabel(simulationReadiness(node))"
        :aria-current="selectedNodeId === node.id ? 'true' : undefined"
        @click="select(node)"
        @keydown="move($event, index)"
      >
        <span class="kind-dot" :data-kind="node.payload.kind"><CircleDot :size="13" /></span>
        <span><strong>{{ node.title }}</strong><small>{{ node.name }}</small></span>
        <AlertTriangle v-if="simulationReadiness(node).level !== 'ready'" class="readiness-icon" :size="12" />
        <code>{{ node.id }}</code>
      </button>
    </div>

    <div v-else class="node-table-wrap">
      <table class="node-table">
        <caption class="sr-only">Visible graph nodes</caption>
        <thead><tr><th scope="col">ID</th><th scope="col">Title</th><th scope="col">Kind</th></tr></thead>
        <tbody>
          <tr v-for="(node, index) in nodes" :key="node.id" :class="{ selected: selectedNodeId === node.id }" :data-readiness="simulationReadiness(node).level">
            <td><code>{{ node.id }}</code></td>
            <th scope="row">
              <button
                :ref="(element) => { if (element) tableButtons[index] = element as HTMLButtonElement }"
                type="button"
                :tabindex="tabIndex(node, index)"
                :aria-current="selectedNodeId === node.id ? 'true' : undefined"
                @click="select(node)"
                @keydown="move($event, index)"
              >{{ node.title }}</button>
            </th>
            <td><AlertTriangle v-if="simulationReadiness(node).level !== 'ready'" class="readiness-icon" :size="10" />{{ node.payload.kind }}</td>
          </tr>
        </tbody>
      </table>
    </div>
    <p v-if="!nodes.length" class="muted">No visible nodes.</p>
  </section>
</template>

<style scoped>
.outline-section { margin-top: var(--space-4); }
.section-title { display: flex; justify-content: space-between; align-items: center; gap: var(--space-2); margin-bottom: 6px; }
.section-title > span:last-child { font: var(--text-xs) var(--mono); color: var(--muted); }
.navigator-view-header { display: grid; grid-template-columns: 1fr auto auto; gap: 8px; align-items: center; }
.navigator-view-tabs { display: flex; padding: 2px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: white; }
.navigator-view-tabs button { width: 27px; height: 25px; display: grid; place-items: center; padding: 0; border: 0; border-radius: 3px; background: transparent; color: var(--muted); }
.navigator-view-tabs button:hover { color: var(--ink); }
.navigator-view-tabs button[aria-pressed='true'] { background: var(--green-soft); color: var(--green); }
.node-outline { display: grid; gap: 1px; }
.node-outline button { display: grid; grid-template-columns: 24px minmax(0, 1fr) auto auto; gap: 8px; align-items: center; width: 100%; padding: 6px 8px 6px 6px; border: 0; border-radius: var(--radius-sm); background: transparent; text-align: left; }
.node-outline button:hover, .node-outline button.selected { background: var(--green-soft); }
.node-outline button[data-readiness='required'] { box-shadow: inset 3px 0 #a83f31; }
.node-outline button[data-readiness='recommended'] { box-shadow: inset 3px 0 #9a6a12; }
.node-outline button > span:nth-child(2) { min-width: 0; display: grid; }
.node-outline strong, .node-outline small { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.node-outline strong { font-size: var(--text-md); }
.node-outline small { margin-top: 1px; color: var(--muted); font-size: var(--text-xs); }
.node-outline code { font: var(--text-xs) var(--mono); color: var(--muted); }
.node-table-wrap { overflow: auto; border: 1px solid var(--line); border-radius: var(--radius-sm); background: white; }
.node-table { width: 100%; border-collapse: collapse; font-size: var(--text-sm); }
.node-table th, .node-table td { padding: 7px; border-bottom: 1px solid #e5e8e2; text-align: left; }
.node-table thead th { color: var(--muted); font-size: var(--text-2xs); text-transform: uppercase; letter-spacing: .06em; }
.node-table tbody tr:last-child > * { border-bottom: 0; }
.node-table tbody tr.selected { background: var(--green-soft); }
.node-table tbody th { padding: 0; font-weight: 600; }
.node-table tbody button { width: 100%; padding: 7px; border: 0; background: transparent; color: var(--ink); text-align: left; font-size: var(--text-sm); }
.node-table code { font: var(--text-xs) var(--mono); color: var(--muted); }
.node-table td:last-child { display: flex; align-items: center; gap: 4px; color: var(--muted); text-transform: capitalize; }
.readiness-icon { color: #9a6a12; }
[data-readiness='required'] .readiness-icon { color: #a83f31; }

@media (max-width: 760px) {
  .outline-section { margin-top: 10px; }
  .node-outline { grid-template-columns: repeat(2, minmax(0, 1fr)); max-height: 112px; overflow: auto; }
}
</style>
