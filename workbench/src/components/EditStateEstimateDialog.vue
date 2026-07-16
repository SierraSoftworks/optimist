<script setup lang="ts">
import { computed, reactive, watch } from 'vue'
import { X } from '@lucide/vue'
import type { GraphNode, SetStateEstimateInput, StateEstimateSlot } from '../api/types'

const props = defineProps<{ open: boolean; pending: boolean; node: GraphNode | null }>()
const emit = defineEmits<{ close: []; submit: [input: SetStateEstimateInput] }>()
const form = reactive({
  slot: 'current' as StateEstimateSlot,
  family: 'point' as 'point' | 'beta',
  value: 0.5,
  alpha: 2,
  beta: 2,
  provenance: '',
})
const existing = computed(() => {
  if (props.node?.payload.kind !== 'factor' && props.node?.payload.kind !== 'outcome') return null
  return props.node.payload.properties[form.slot]
})

watch(
  () => [props.open, props.node, form.slot] as const,
  ([open]) => {
    if (!open) return
    const distribution = existing.value?.distribution
    form.provenance = existing.value?.provenance?.join('\n') ?? ''
    if (distribution?.type === 'beta') {
      form.family = 'beta'
      form.alpha = distribution.alpha ?? 2
      form.beta = distribution.beta ?? 2
    } else {
      form.family = 'point'
      form.value = distribution?.value ?? 0.5
    }
  },
)

function submit() {
  emit('submit', {
    slot: form.slot,
    distribution:
      form.family === 'point'
        ? { type: 'point', value: form.value }
        : { type: 'beta', alpha: form.alpha, beta: form.beta },
    provenance: form.provenance
      .split('\n')
      .map((value) => value.trim())
      .filter(Boolean),
  })
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open && node" class="dialog-backdrop" @click.self="emit('close')">
      <form class="dialog estimate-dialog" aria-labelledby="edit-estimate-title" @submit.prevent="submit">
        <header>
          <div>
            <span class="eyebrow">{{ node.title }}</span>
            <h2 id="edit-estimate-title">Set state estimate</h2>
          </div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>
        <div class="field-grid">
          <label>
            State
            <select v-model="form.slot">
              <option value="current">Current</option>
              <option value="desired">Desired</option>
            </select>
          </label>
          <label>
            Distribution
            <select v-model="form.family">
              <option value="point">Point</option>
              <option value="beta">Beta</option>
            </select>
          </label>
        </div>
        <label v-if="form.family === 'point'">
          Value on [0, 1]
          <input v-model.number="form.value" type="number" min="0" max="1" step="0.01" required />
        </label>
        <div v-else class="field-grid">
          <label>
            Alpha
            <input v-model.number="form.alpha" type="number" min="0.01" step="0.1" required />
          </label>
          <label>
            Beta
            <input v-model.number="form.beta" type="number" min="0.01" step="0.1" required />
          </label>
        </div>
        <label>
          Provenance
          <textarea v-model="form.provenance" rows="4" placeholder="One source or elicitation note per line"></textarea>
        </label>
        <footer>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending">
            {{ pending ? 'Saving…' : existing ? 'Replace estimate' : 'Set estimate' }}
          </button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>
