<script setup lang="ts">
import { reactive, watch } from 'vue'
import { X } from '@lucide/vue'
import type { AppendObservationInput, GraphEdge } from '../api/types'

const props = defineProps<{
  open: boolean
  pending: boolean
  edge: GraphEdge | null
  unit: string
}>()
const emit = defineEmits<{ close: []; submit: [input: AppendObservationInput] }>()
const form = reactive({
  value: 0,
  observedAt: '',
  source: '',
  uncertaintyEnabled: false,
  standardDeviation: 0,
})

function localDateTime(date: Date) {
  const offset = date.getTimezoneOffset() * 60_000
  return new Date(date.getTime() - offset).toISOString().slice(0, 16)
}

watch(
  () => props.open,
  (open) => {
    if (!open) return
    Object.assign(form, {
      value: 0,
      observedAt: localDateTime(new Date()),
      source: '',
      uncertaintyEnabled: false,
      standardDeviation: 0,
    })
  },
)

function submit() {
  const observedAt = new Date(form.observedAt)
  if (!Number.isFinite(form.value) || Number.isNaN(observedAt.getTime()) || !form.source.trim()) return
  emit('submit', {
    value: form.value,
    unit: props.unit,
    observed_at: observedAt.toISOString(),
    source: form.source.trim(),
    measurement_standard_deviation: form.uncertaintyEnabled ? form.standardDeviation : null,
  })
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open && edge" class="dialog-backdrop" @pointerdown.self="emit('close')">
      <form class="dialog" aria-labelledby="add-observation-title" @submit.prevent="submit">
        <header>
          <div>
            <span class="eyebrow">{{ edge.source }} measures {{ edge.destination }}</span>
            <h2 id="add-observation-title">Add observation</h2>
          </div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>
        <div class="field-grid">
          <label>Value<input v-model.number="form.value" type="number" step="any" required /></label>
          <label>Unit<input :value="unit" readonly /></label>
        </div>
        <label>Observed at<input v-model="form.observedAt" type="datetime-local" required /></label>
        <label>Source<input v-model="form.source" placeholder="Dashboard, query, person, or citation" required /></label>
        <label class="checkbox-label"><input v-model="form.uncertaintyEnabled" type="checkbox" /> Include known measurement error</label>
        <label v-if="form.uncertaintyEnabled">Standard deviation<input v-model.number="form.standardDeviation" type="number" min="0" step="any" required /></label>
        <footer>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending || !form.source.trim()">{{ pending ? 'Adding…' : 'Add observation' }}</button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>
