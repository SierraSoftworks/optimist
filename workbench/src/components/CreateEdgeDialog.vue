<script setup lang="ts">
import { computed, reactive, watch } from 'vue'
import { X } from '@lucide/vue'
import type { CreateEdgeInput, GraphNode } from '../api/types'

const props = defineProps<{ open: boolean; pending: boolean; nodes: GraphNode[] }>()
const emit = defineEmits<{ close: []; submit: [input: CreateEdgeInput] }>()
const form = reactive({ source: '', destination: '', kind: 'part_of' as 'part_of' | 'requires', hard: true })

const validSources = computed(() =>
  props.nodes.filter((node) =>
    form.kind === 'part_of'
      ? node.payload.kind === 'factor'
      : ['factor', 'intervention'].includes(node.payload.kind),
  ),
)

const validDestinations = computed(() =>
  props.nodes.filter((node) => {
    if (node.id === form.source) return false
    const source = props.nodes.find((candidate) => candidate.id === form.source)
    if (!source) return false
    if (form.kind === 'part_of') return source.payload.kind === 'factor' && node.payload.kind === 'factor'
    return (
      ['factor', 'intervention'].includes(source.payload.kind) &&
      ['factor', 'intervention'].includes(node.payload.kind)
    )
  }),
)

watch(
  () => props.open,
  (open) => {
    if (!open) return
    form.source = ''
    form.destination = ''
    form.kind = 'part_of'
    form.hard = true
  },
)

watch([() => form.source, () => form.kind], () => {
  if (!validSources.value.some((node) => node.id === form.source)) form.source = ''
  if (!validDestinations.value.some((node) => node.id === form.destination)) form.destination = ''
})

function submit() {
  if (!form.source || !form.destination) return
  emit('submit', {
    source: form.source,
    destination: form.destination,
    payload:
      form.kind === 'part_of'
        ? { kind: 'part_of' }
        : {
            kind: 'requires',
            properties: { hard: form.hard, satisfaction_threshold: null },
          },
  })
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="dialog-backdrop" @click.self="emit('close')">
      <form class="dialog" aria-labelledby="create-edge-title" @submit.prevent="submit">
        <header>
          <div>
            <span class="eyebrow">Graph structure</span>
            <h2 id="create-edge-title">Add relationship</h2>
          </div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>

        <label>
          Relationship
          <select v-model="form.kind">
            <option value="part_of">Part of</option>
            <option value="requires">Requires</option>
          </select>
        </label>
        <div class="field-grid relationship-fields">
          <label>
            Source
            <select v-model="form.source" required>
              <option value="" disabled>Select node</option>
              <option v-for="node in validSources" :key="node.id" :value="node.id">{{ node.title }} · {{ node.id }}</option>
            </select>
          </label>
          <label>
            Destination
            <select v-model="form.destination" required>
              <option value="" disabled>Select node</option>
              <option v-for="node in validDestinations" :key="node.id" :value="node.id">{{ node.title }} · {{ node.id }}</option>
            </select>
          </label>
        </div>
        <label v-if="form.kind === 'requires'" class="checkbox-label">
          <input v-model="form.hard" type="checkbox" />
          Hard prerequisite
        </label>

        <p v-if="validSources.length === 0" class="form-note">Add at least two factors or interventions first.</p>
        <footer>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending || !form.destination">
            {{ pending ? 'Adding…' : 'Add relationship' }}
          </button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>
