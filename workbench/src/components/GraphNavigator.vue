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
