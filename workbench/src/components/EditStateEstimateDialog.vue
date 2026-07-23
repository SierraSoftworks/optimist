<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { X } from '@lucide/vue'
import type { EstimateSourceInput, EstimateSupport, GraphEdge, GraphNode, QuantitySupport, SetStateEstimateInput, StateEstimateSlot, Unit } from '../api/types'
import { defaultSquiggleSourceInput } from '../domain/squiggleEstimate'
import EstimateSourceEditor from './EstimateSourceEditor.vue'

const props = defineProps<{
  open: boolean
  pending: boolean
  node: GraphNode | null
  projectId: string | null
  edges: GraphEdge[]
}>()
const emit = defineEmits<{ close: []; submit: [input: SetStateEstimateInput] }>()
const form = reactive({
  slot: 'current' as StateEstimateSlot,
})
const source = ref<EstimateSourceInput>(defaultSquiggleSourceInput('probability', {}))
const sourceValid = ref(true)
const existing = computed(() => {
  if (props.node?.native_state) {
    return form.slot === 'current'
      ? props.node.native_state.current ?? null
      : props.node.native_state.forecast ?? null
  }
  if (props.node?.payload.kind === 'metric') return props.node.payload.properties.current ?? null
  return null
})
const metricSupport = computed<QuantitySupport>(() =>
  props.node?.native_state
    ? props.node.native_state.quantity.support ?? { type: 'real' }
    : props.node?.payload.kind === 'metric'
    ? props.node.payload.properties.quantity.support ?? { type: 'real' }
    : { type: 'real' },
)
const estimateSupport = computed<EstimateSupport>(() => {
  if (metricSupport.value.type === 'bounded') {
    return { bounded: { lower: metricSupport.value.lower, upper: metricSupport.value.upper } }
  }
  return metricSupport.value.type
})
const expectedUnit = computed<Unit>(() =>
  props.node?.native_state
    ? props.node.native_state.quantity.dimension ?? {}
    : props.node?.payload.kind === 'metric' ? props.node.payload.properties.quantity.dimension ?? {} : {},
)
const canAuthor = computed(() =>
  props.node?.native_state
    ? props.node.native_state.quantity.dimension !== undefined
    : props.node?.payload.kind === 'metric' && props.node.payload.properties.quantity.dimension !== undefined,
)

watch(
  () => [props.open, props.node, form.slot] as const,
  ([open]) => {
    if (!open) return
    if (props.node?.payload.kind === 'metric') form.slot = 'current'
    if (existing.value) {
      source.value = { type: 'squiggle', definition: existing.value.source.definition }
    } else {
      source.value = defaultSquiggleSourceInput(estimateSupport.value, expectedUnit.value)
    }
    sourceValid.value = true
  },
  { immediate: true },
)

function submit() {
  emit('submit', {
    slot: form.slot,
    source: source.value,
    provenance: existing.value?.provenance ?? [],
    uncertainty: existing.value?.uncertainty,
  })
}

</script>

<template>
  <Teleport to="body">
    <div v-if="open && node" class="dialog-backdrop" @pointerdown.self="emit('close')">
      <form class="dialog estimate-dialog" aria-labelledby="edit-estimate-title" @submit.prevent="submit">
        <header>
          <div>
            <span class="eyebrow">{{ node.title }}</span>
            <h2 id="edit-estimate-title">{{ node.payload.kind === 'metric' ? 'Set quantity estimate' : 'Set state estimate' }}</h2>
          </div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>
        <label v-if="node.payload.kind !== 'metric'">
          State
          <select v-model="form.slot">
            <option value="current">Current</option>
            <option value="forecast">Forecast</option>
          </select>
        </label>
        <EstimateSourceEditor
          v-if="canAuthor"
          v-model="source"
          :existing="existing"
          :support="estimateSupport"
          :project-id="projectId"
          :expected-unit="expectedUnit"
          @validity="sourceValid = $event"
        />
        <p v-else class="form-error">Configure a canonical quantity before authoring a Squiggle estimate.</p>
        <footer>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending || !sourceValid || !canAuthor">
            {{ pending ? 'Saving…' : existing ? 'Replace estimate' : 'Set estimate' }}
          </button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>

<style scoped>
.calibrated-evidence { display: grid; gap: 8px; padding: 10px; border: 1px solid #a8bfb2; border-radius: 6px; background: #f3f8f4; }
.calibrated-evidence > div { display: flex; justify-content: space-between; gap: 8px; }
.calibrated-evidence > div strong { font-size: 10px; }
.calibrated-evidence > div span { color: var(--muted); font-size: 8px; }
.calibrated-evidence article { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 10px; align-items: center; padding-top: 8px; border-top: 1px solid #cbd9d0; }
.calibrated-evidence article > div { min-width: 0; display: grid; gap: 2px; }
.calibrated-evidence article strong { font-size: 10px; }
.calibrated-evidence article span, .calibrated-evidence article small { color: var(--muted); font-size: 8px; line-height: 1.4; }

@media (max-width: 760px) {
  .calibrated-evidence article { grid-template-columns: 1fr; }
  .calibrated-evidence article .secondary-button { justify-self: start; }
}
</style>
