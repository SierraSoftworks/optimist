<script setup lang="ts">
import { reactive, watch } from 'vue'
import { X } from '@lucide/vue'
import type { CorrectObservationInput, GraphEdge, Observation } from '../api/types'

const props = defineProps<{
  open: boolean
  pending: boolean
  edge: GraphEdge | null
  observation: Observation | null
}>()
const emit = defineEmits<{ close: []; submit: [input: CorrectObservationInput] }>()
const form = reactive({ value: 0 })

watch(
  () => [props.open, props.observation] as const,
  ([open, observation]) => {
    if (open && observation) form.value = observation.value
  },
)

function submit() {
  if (!props.observation || !Number.isFinite(form.value)) return
  emit('submit', { observation_id: props.observation.id, value: form.value })
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open && edge && observation" class="dialog-backdrop" @pointerdown.self="emit('close')">
      <form class="dialog" aria-labelledby="correct-observation-title" @submit.prevent="submit">
        <header>
          <div>
            <span class="eyebrow">Observation #{{ observation.id }} · {{ edge.source }} measures {{ edge.destination }}</span>
            <h2 id="correct-observation-title">Correct observation</h2>
          </div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>
        <p class="form-note correction-note">The original reading remains in history. This appends a correction with the same unit, timestamp, source, and measurement error.</p>
        <label>Corrected value<input v-model.number="form.value" type="number" step="any" required /></label>
        <dl class="correction-context">
          <div><dt>Unit</dt><dd>{{ observation.unit }}</dd></div>
          <div><dt>Observed</dt><dd>{{ new Date(observation.observed_at).toLocaleString() }}</dd></div>
          <div><dt>Source</dt><dd>{{ observation.source }}</dd></div>
          <div><dt>Standard deviation</dt><dd>{{ observation.measurement_standard_deviation ?? 'Unknown' }}</dd></div>
        </dl>
        <footer>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending">{{ pending ? 'Correcting…' : 'Append correction' }}</button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>

<style scoped>
.correction-note { margin: 0 0 16px; }
.correction-context { margin-top: 16px; padding: 10px; border: 1px solid var(--line); border-radius: 5px; background: #f7f9f5; }
</style>
