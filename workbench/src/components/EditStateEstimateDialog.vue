<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { X } from '@lucide/vue'
import type { EstimateSourceInput, EstimateSupport, EstimateUncertainty, GraphEdge, GraphNode, QuantitySupport, SetStateEstimateInput, StateEstimateSlot, Unit } from '../api/types'
import { defaultSquiggleSourceInput, squiggleSourceInput } from '../domain/squiggleEstimate'
import EstimateSourceEditor from './EstimateSourceEditor.vue'
import EstimateUncertaintyEditor from './EstimateUncertaintyEditor.vue'
import { calibratedState, calibrationLabel, latestObservation } from '../domain/measurementCalibration'

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
  provenance: '',
})
const source = ref<EstimateSourceInput>(defaultSquiggleSourceInput('probability', {}))
const uncertainty = ref<EstimateUncertainty>({})
const sourceValid = ref(true)
const existing = computed(() => {
  if (props.node?.native_state) {
    return form.slot === 'current'
      ? props.node.native_state.current ?? null
      : props.node.native_state.forecast ?? null
  }
  if (props.node?.payload.kind === 'metric') return props.node.payload.properties.current ?? null
  if (props.node?.payload.kind !== 'factor' && props.node?.payload.kind !== 'outcome') return null
  return props.node.payload.properties[form.slot]
})
const metricSupport = computed<QuantitySupport>(() =>
  props.node?.native_state
    ? props.node.native_state.quantity.support ?? { type: 'real' }
    : props.node?.payload.kind === 'metric'
    ? props.node.payload.properties.support ?? { type: 'real' }
    : { type: 'bounded', lower: 0, upper: 1 },
)
const estimateSupport = computed<EstimateSupport>(() => {
  if (!props.node?.native_state && props.node?.payload.kind !== 'metric') return 'probability'
  if (metricSupport.value.type === 'bounded') {
    return { bounded: { lower: metricSupport.value.lower, upper: metricSupport.value.upper } }
  }
  return metricSupport.value.type
})
const expectedUnit = computed<Unit>(() =>
  props.node?.native_state
    ? props.node.native_state.quantity.dimension ?? {}
    : props.node?.payload.kind === 'metric' ? props.node.payload.properties.dimension ?? {} : {},
)
const canAuthor = computed(() =>
  props.node?.native_state
    ? props.node.native_state.quantity.dimension !== undefined
    : props.node?.payload.kind !== 'metric' || props.node.payload.properties.dimension !== undefined,
)
const calibratedReadings = computed(() => {
  if (props.node?.native_state || (props.node?.payload.kind !== 'factor' && props.node?.payload.kind !== 'outcome')) return []
  return props.edges.flatMap((edge) => {
    if (edge.destination !== props.node?.id || edge.payload.kind !== 'measures') return []
    const calibration = edge.payload.properties.calibration
    const observation = latestObservation(edge.payload.properties.observations)
    if (!calibration || !observation) return []
    const state = calibratedState(calibration, observation.value)
    return state === null ? [] : [{ edge, calibration, observation, state }]
  })
})

watch(
  () => [props.open, props.node, form.slot] as const,
  ([open]) => {
    if (!open) return
    if (props.node?.payload.kind === 'metric') form.slot = 'current'
    const currentDistribution = existing.value?.distribution
    form.provenance = existing.value?.provenance?.join('\n') ?? ''
    uncertainty.value = { ...existing.value?.uncertainty }
    if (currentDistribution) {
      source.value = existing.value?.source?.type === 'fermi'
        ? { type: 'fermi', definition: existing.value.source.definition }
        : existing.value?.source?.type === 'squiggle'
          ? { type: 'squiggle', definition: existing.value.source.definition }
          : { type: 'distribution', distribution: currentDistribution }
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
    provenance: form.provenance
      .split('\n')
      .map((value) => value.trim())
      .filter(Boolean),
    uncertainty: uncertainty.value,
  })
}

function useReading(reading: typeof calibratedReadings.value[number]) {
  source.value = squiggleSourceInput(`pointMass(${reading.state})`, expectedUnit.value)
  sourceValid.value = true
  form.provenance = [
    form.provenance.trim(),
    `Calibrated observation #${reading.observation.id}: ${reading.observation.value} ${reading.observation.unit} from ${reading.observation.source} at ${reading.observation.observed_at} mapped to normalized state ${reading.state.toFixed(4)}.`,
  ].filter(Boolean).join('\n')
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open && node" class="dialog-backdrop" @click.self="emit('close')">
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
            <option value="desired">{{ node.native_state ? 'Forecast' : 'Desired' }}</option>
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
        <p v-else class="form-error">Add canonical unit terms to this legacy metric before authoring a Squiggle estimate.</p>
        <section v-if="calibratedReadings.length" class="calibrated-evidence">
          <div><strong>Metric evidence</strong><span>Latest unsuperseded readings</span></div>
          <article v-for="reading in calibratedReadings" :key="`${reading.edge.source}-${reading.observation.id}`">
            <div>
              <strong>{{ reading.observation.value }} {{ reading.observation.unit }} → {{ reading.state.toFixed(3) }}</strong>
              <span>{{ calibrationLabel(reading.calibration, reading.observation.unit) }}</span>
              <small>{{ new Date(reading.observation.observed_at).toLocaleString() }} · {{ reading.observation.source }}</small>
            </div>
            <button type="button" class="secondary-button" @click="useReading(reading)">Use reading</button>
          </article>
        </section>
        <EstimateUncertaintyEditor v-model="uncertainty" />
        <label>
          Provenance
          <textarea v-model="form.provenance" rows="4" placeholder="One source or elicitation note per line"></textarea>
        </label>
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
