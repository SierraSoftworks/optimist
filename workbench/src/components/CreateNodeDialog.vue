<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from 'vue'
import { X } from '@lucide/vue'
import type { CreateNodeInput, NodeKind, NodePayload } from '../api/types'

const props = defineProps<{ open: boolean; pending: boolean }>()
const emit = defineEmits<{ close: []; submit: [input: CreateNodeInput] }>()
const titleInput = ref<HTMLInputElement>()
const form = reactive({
  kind: 'factor' as NodeKind,
  title: '',
  name: '',
  direction: 'maximize' as 'maximize' | 'minimize' | 'target_range',
  unit: '',
  controllable: false,
})

const generatedName = computed(() =>
  form.title
    .trim()
    .toLocaleLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_|_$/g, ''),
)

watch(
  () => props.open,
  async (open) => {
    if (!open) return
    Object.assign(form, {
      kind: 'factor',
      title: '',
      name: '',
      direction: 'maximize',
      unit: '',
      controllable: false,
    })
    await nextTick()
    titleInput.value?.focus()
  },
)

function payload(): NodePayload {
  switch (form.kind) {
    case 'outcome':
      return {
        kind: 'outcome',
        properties: {
          direction: form.direction,
          current: null,
          desired: null,
          evidence: [],
        },
      }
    case 'metric':
      return { kind: 'metric', properties: { unit: form.unit.trim(), aggregation: null } }
    case 'intervention':
      return {
        kind: 'intervention',
        properties: {
          costs: [],
          duration: null,
          probability_of_success: null,
          acceptance_criteria: [],
        },
      }
    default:
      return {
        kind: 'factor',
        properties: {
          current: null,
          desired: null,
          controllable: form.controllable,
          evidence: [],
        },
      }
  }
}

function submit() {
  const title = form.title.trim()
  const name = form.name.trim() || generatedName.value
  if (!title || !name || (form.kind === 'metric' && !form.unit.trim())) return
  emit('submit', { name, title, payload: payload() })
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="dialog-backdrop" @click.self="emit('close')">
      <form class="dialog node-dialog" aria-labelledby="create-node-title" @submit.prevent="submit">
        <header>
          <div>
            <span class="eyebrow">Graph element</span>
            <h2 id="create-node-title">Add node</h2>
          </div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>

        <fieldset>
          <legend>Node kind</legend>
          <div class="kind-options">
            <label v-for="kind in (['outcome', 'metric', 'factor', 'intervention'] as NodeKind[])" :key="kind">
              <input v-model="form.kind" type="radio" name="kind" :value="kind" />
              <span :data-kind="kind">{{ kind }}</span>
            </label>
          </div>
        </fieldset>

        <div class="field-grid">
          <label>
            Title
            <input ref="titleInput" v-model="form.title" placeholder="Fast feedback" required />
          </label>
          <label>
            Name
            <input v-model="form.name" :placeholder="generatedName || 'fast_feedback'" />
          </label>
        </div>

        <label v-if="form.kind === 'outcome'">
          Preferred direction
          <select v-model="form.direction">
            <option value="maximize">Maximize</option>
            <option value="minimize">Minimize</option>
            <option value="target_range">Target range</option>
          </select>
        </label>
        <label v-if="form.kind === 'metric'">
          Unit
          <input v-model="form.unit" placeholder="minutes" required />
        </label>
        <label v-if="form.kind === 'factor'" class="checkbox-label">
          <input v-model="form.controllable" type="checkbox" />
          Directly controllable
        </label>

        <footer>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending">
            {{ pending ? 'Adding…' : 'Add node' }}
          </button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>
