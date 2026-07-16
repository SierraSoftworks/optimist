<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { X } from '@lucide/vue'
import type { EstimateSourceInput, GraphEdge, GraphNode, SetStateEstimateInput, StateEstimateSlot } from '../api/types'
import EstimateSourceEditor from './EstimateSourceEditor.vue'
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
const source = ref<EstimateSourceInput>({ type: 'distribution', distribution: { type: 'point', value: 0.5 } })
const sourceValid = ref(true)
const existing = computed(() => {
  if (props.node?.payload.kind !== 'factor' && props.node?.payload.kind !== 'outcome') return null
  return props.node.payload.properties[form.slot]
})
const calibratedReadings = computed(() => {
  if (props.node?.payload.kind !== 'factor' && props.node?.payload.kind !== 'outcome') return []
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
    const currentDistribution = existing.value?.distribution
    form.provenance = existing.value?.provenance?.join('\n') ?? ''
    if (currentDistribution?.type === 'beta' || currentDistribution?.type === 'point') {
      source.value = existing.value?.source?.type === 'fermi'
        ? { type: 'fermi', definition: existing.value.source.definition }
        : { type: 'distribution', distribution: currentDistribution }
    } else source.value = { type: 'distribution', distribution: { type: 'point', value: 0.5 } }
    sourceValid.value = true
  },
)

function submit() {
  emit('submit', {
    slot: form.slot,
    source: source.value,
    provenance: form.provenance
      .split('\n')
      .map((value) => value.trim())
      .filter(Boolean),
  })
}

function useReading(reading: typeof calibratedReadings.value[number]) {
  source.value = { type: 'distribution', distribution: { type: 'point', value: reading.state } }
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
            <h2 id="edit-estimate-title">Set state estimate</h2>
          </div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>
        <label>
          State
          <select v-model="form.slot">
            <option value="current">Current</option>
            <option value="desired">Desired</option>
          </select>
        </label>
        <EstimateSourceEditor
          v-model="source"
          :existing="existing"
          :families="['point', 'beta']"
          support="probability"
          point-label="Value on [0, 1]"
          :project-id="projectId"
          :expected-unit="{}"
          @validity="sourceValid = $event"
        />
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
        <label>
          Provenance
          <textarea v-model="form.provenance" rows="4" placeholder="One source or elicitation note per line"></textarea>
        </label>
        <footer>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending || !sourceValid">
            {{ pending ? 'Saving…' : existing ? 'Replace estimate' : 'Set estimate' }}
          </button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>
