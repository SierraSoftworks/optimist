<script setup lang="ts">
import { reactive, ref, watch } from 'vue'
import { Pencil, Trash2, X } from '@lucide/vue'
import type {
  Distribution,
  EdgeEstimateSlot,
  GraphEdge,
  MeasurementCalibration,
  SetMeasurementCalibrationInput,
} from '../api/types'
import { calibratedState, calibrationLabel } from '../domain/measurementCalibration'
import { formatUnitExpression } from '../domain/unitExpression'

const props = defineProps<{ open: boolean; pending: boolean; edge: GraphEdge | null }>()
const emit = defineEmits<{
  close: []
  delete: []
  estimate: [slot: EdgeEstimateSlot]
  calibration: [input: SetMeasurementCalibrationInput]
}>()
const calibration = reactive({
  enabled: false,
  stateZero: 0,
  stateOne: 1,
  outerLower: 0,
  idealLower: 0.4,
  idealUpper: 0.6,
  outerUpper: 1,
})
const confirmDelete = ref(false)

watch(
  () => [props.open, props.edge] as const,
  ([open, edge]) => {
    if (!open || !edge) return
    if (edge.payload.kind === 'measures') {
      const current = edge.payload.properties.calibration
      calibration.enabled = Boolean(current)
      if (current?.type === 'linear') {
        calibration.stateZero = current.state_zero
        calibration.stateOne = current.state_one
      } else if (current?.type === 'target_range') {
        calibration.outerLower = current.outer_lower
        calibration.idealLower = current.ideal_lower
        calibration.idealUpper = current.ideal_upper
        calibration.outerUpper = current.outer_upper
      }
    }
    confirmDelete.value = false
  },
)

function calibrationValue(): MeasurementCalibration | null {
  if (!calibration.enabled || props.edge?.payload.kind !== 'measures') return null
  if (props.edge.payload.properties.polarity === 'target_range') {
    return {
      type: 'target_range',
      outer_lower: calibration.outerLower,
      ideal_lower: calibration.idealLower,
      ideal_upper: calibration.idealUpper,
      outer_upper: calibration.outerUpper,
    }
  }
  return { type: 'linear', state_zero: calibration.stateZero, state_one: calibration.stateOne }
}

function saveCalibration() {
  emit('calibration', { calibration: calibrationValue() })
}

function sampleReadings(value: MeasurementCalibration) {
  if (value.type === 'linear') {
    return [value.state_zero, (value.state_zero + value.state_one) / 2, value.state_one]
  }
  return [value.outer_lower, (value.ideal_lower + value.ideal_upper) / 2, value.outer_upper]
}

function distributionLabel(value: Distribution) {
  if (value.type === 'point') return `Point · ${value.value}`
  if (value.type === 'beta') return `Beta · α ${value.alpha}, β ${value.beta}`
  if (value.type === 'scaled_beta') return `Scaled Beta · [${value.lower}, ${value.upper}]`
  if (value.type === 'normal') return `Normal · μ ${value.mean}, σ ${value.standard_deviation}`
  if (value.type === 'empirical') return `Empirical · ${(value.samples ?? []).length.toLocaleString()} samples`
  return `LogNormal · μ ${value.location}, σ ${value.scale}`
}

</script>

<template>
  <Teleport to="body">
    <div v-if="open && edge" class="dialog-backdrop" @click.self="emit('close')">
      <section class="dialog edge-edit-dialog" role="dialog" aria-labelledby="edit-edge-title">
        <header>
          <div>
            <span class="eyebrow">{{ edge.source }} · {{ edge.payload.kind.replaceAll('_', ' ') }} · {{ edge.destination }}</span>
            <h2 id="edit-edge-title">Edit relationship</h2>
          </div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>
        <section v-if="edge.payload.kind === 'contributes' || edge.payload.kind === 'changes'" class="dialog-section">
          <div v-if="edge.payload.properties.effect" class="estimate-row">
            <div><span>Effect</span><strong>{{ distributionLabel(edge.payload.properties.effect.distribution) }}</strong></div>
            <button type="button" class="icon-button" aria-label="Edit relationship effect estimate" @click="emit('estimate', { kind: 'effect' })"><Pencil :size="13" /></button>
          </div>
          <div v-if="edge.payload.properties.response" class="estimate-row">
            <div><span>Counterfactual response</span><strong>{{ edge.payload.properties.response.source_change }} {{ formatUnitExpression(edge.payload.properties.response.source_unit) }} → {{ distributionLabel(edge.payload.properties.response.destination_change.distribution) }} {{ formatUnitExpression(edge.payload.properties.response.destination_unit) }}</strong></div>
            <button type="button" class="icon-button" aria-label="Edit destination response estimate" @click="emit('estimate', { kind: 'response' })"><Pencil :size="13" /></button>
          </div>
          <div class="estimate-row">
            <div><span>Lag</span><strong>{{ edge.payload.properties.lag ? distributionLabel(edge.payload.properties.lag.distribution) : 'Not set' }}</strong></div>
            <button type="button" class="icon-button" aria-label="Edit relationship lag estimate" @click="emit('estimate', { kind: 'lag' })"><Pencil :size="13" /></button>
          </div>
        </section>
        <section v-else-if="edge.payload.kind === 'blocks'" class="dialog-section">
          <div class="estimate-row">
            <div><span>Blocking degree</span><strong>{{ distributionLabel(edge.payload.properties.degree.distribution) }}</strong></div>
            <button type="button" class="icon-button" aria-label="Edit blocking degree estimate" @click="emit('estimate', { kind: 'degree' })"><Pencil :size="13" /></button>
          </div>
        </section>
        <section v-else-if="edge.payload.kind === 'measures'" class="dialog-section calibration-editor">
          <header class="section-header">
            <div><strong>Metric to state</strong><span>{{ edge.payload.properties.polarity.replaceAll('_', ' ') }}</span></div>
            <label class="checkbox-label"><input v-model="calibration.enabled" type="checkbox" /> Calibrated</label>
          </header>
          <template v-if="calibration.enabled && edge.payload.properties.polarity !== 'target_range'">
            <div class="field-grid">
              <label>Reading at state 0<input v-model.number="calibration.stateZero" type="number" step="any" /></label>
              <label>Reading at state 1<input v-model.number="calibration.stateOne" type="number" step="any" /></label>
            </div>
          </template>
          <template v-else-if="calibration.enabled">
            <div class="calibration-fields">
              <label>Outer low<input v-model.number="calibration.outerLower" type="number" step="any" /></label>
              <label>Ideal low<input v-model.number="calibration.idealLower" type="number" step="any" /></label>
              <label>Ideal high<input v-model.number="calibration.idealUpper" type="number" step="any" /></label>
              <label>Outer high<input v-model.number="calibration.outerUpper" type="number" step="any" /></label>
            </div>
          </template>
          <template v-if="calibration.enabled && calibrationValue()">
            <p class="calibration-summary">{{ calibrationLabel(calibrationValue()!, 'metric units') }}</p>
            <div class="calibration-preview" aria-label="Calibration preview">
              <div v-for="reading in sampleReadings(calibrationValue()!)" :key="reading">
                <span>{{ reading }}</span><strong>{{ calibratedState(calibrationValue()!, reading)?.toFixed(2) }}</strong>
              </div>
            </div>
          </template>
          <p v-else class="muted">Polarity describes direction only. Add anchors to translate readings into normalized state estimates.</p>
          <button type="button" class="secondary-button" :disabled="pending" @click="saveCalibration">{{ calibration.enabled ? 'Save calibration' : 'Remove calibration' }}</button>
        </section>
        <div v-if="confirmDelete" class="replace-warning">
          <Trash2 :size="18" />
          <div><strong>Delete this relationship?</strong><span>The endpoint nodes will remain in the project.</span></div>
        </div>
        <footer>
          <button
            type="button"
            class="danger-button"
            :disabled="pending"
            @click="confirmDelete ? emit('delete') : (confirmDelete = true)"
          ><Trash2 :size="14" /> {{ confirmDelete ? 'Confirm delete' : 'Delete' }}</button>
          <span class="footer-spacer"></span>
          <button type="button" class="primary-button" @click="emit('close')">Done</button>
        </footer>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.dialog-section { margin: 0 0 16px; padding: 0 0 16px; border-bottom: 1px solid var(--line); }
.calibration-editor { display: grid; gap: 10px; }
.calibration-editor .section-header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.calibration-editor .section-header > div { display: grid; gap: 2px; }
.calibration-editor .section-header strong { font-size: 11px; }
.calibration-editor .section-header span { color: var(--muted); font-size: 9px; text-transform: capitalize; }
.calibration-fields { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 7px; }
.calibration-summary { margin: 0; color: var(--muted); font-size: 9px; line-height: 1.45; }
.calibration-preview { display: grid; grid-template-columns: repeat(3, 1fr); overflow: hidden; border: 1px solid var(--line); border-radius: 5px; }
.calibration-preview div { display: grid; gap: 2px; padding: 7px; border-right: 1px solid var(--line); text-align: center; }
.calibration-preview div:last-child { border-right: 0; }
.calibration-preview span { color: var(--muted); font-size: 8px; }
.calibration-preview strong { color: var(--green); font: 10px 'IBM Plex Mono', monospace; }
.calibration-editor > .secondary-button { justify-self: end; }

@media (max-width: 760px) {
  .calibration-fields { grid-template-columns: repeat(2, minmax(0, 1fr)); }
}
</style>
