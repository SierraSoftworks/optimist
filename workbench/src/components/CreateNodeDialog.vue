<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from 'vue'
import { ArrowLeft, ArrowRight, Check, X } from '@lucide/vue'
import type {
  CreateNodeInput,
  Distribution,
  Estimate,
  NodeKind,
  NodePayload,
  QuantitySupport,
} from '../api/types'
import DistributionEditor from './DistributionEditor.vue'
import { parseUnitExpression } from '../domain/unitExpression'

const props = defineProps<{ open: boolean; pending: boolean }>()
const emit = defineEmits<{ close: []; submit: [input: CreateNodeInput] }>()
const titleInput = ref<HTMLInputElement>()
const step = ref<1 | 2>(1)
const currentState = ref<Distribution>({ type: 'beta', alpha: 2, beta: 2 })
const successProbability = ref<Distribution>({ type: 'beta', alpha: 4, beta: 2 })
const duration = ref<Distribution>({ type: 'log_normal', location: Math.log(4), scale: 0.35 })
const metricCurrent = ref<Distribution>({ type: 'point', value: 0 })
const form = reactive({
  kind: 'factor' as NodeKind,
  title: '',
  name: '',
  direction: 'maximize' as 'maximize' | 'minimize' | 'target_range',
  unit: '',
  aggregation: '',
  metricSupport: 'real' as 'real' | 'non_negative' | 'bounded',
  metricLower: 0,
  metricUpper: 1,
  operationalDefinition: '',
  referenceTime: '',
  resolutionSource: '',
  metricHasEstimate: false,
  controllable: false,
  provenance: '',
  acceptanceCriteria: '',
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
      aggregation: '',
      metricSupport: 'real',
      metricLower: 0,
      metricUpper: 1,
      operationalDefinition: '',
      referenceTime: '',
      resolutionSource: '',
      metricHasEstimate: false,
      controllable: false,
      provenance: '',
      acceptanceCriteria: '',
    })
    currentState.value = { type: 'beta', alpha: 2, beta: 2 }
    successProbability.value = { type: 'beta', alpha: 4, beta: 2 }
    duration.value = { type: 'log_normal', location: Math.log(4), scale: 0.35 }
    metricCurrent.value = { type: 'point', value: 0 }
    step.value = 1
    await nextTick()
    titleInput.value?.focus()
  },
)

const identityValid = computed(() =>
  Boolean(
    form.title.trim() &&
    (form.name.trim() || generatedName.value) &&
    (form.kind !== 'metric' || metricDimension.value),
  ),
)
const metricDimension = computed(() => {
  if (form.kind !== 'metric') return null
  try {
    return parseUnitExpression(form.unit)
  } catch {
    return null
  }
})

const setupTitle = computed(() => {
  if (form.kind === 'metric') return 'Measurement setup'
  if (form.kind === 'intervention') return 'Planning estimates'
  return 'Simulation baseline'
})
const metricBoundsValid = computed(() =>
  form.metricSupport !== 'bounded' || (
    Number.isFinite(form.metricLower) &&
    Number.isFinite(form.metricUpper) &&
    form.metricLower < form.metricUpper
  ),
)
const metricFamilies = computed<Distribution['type'][]>(() => {
  if (form.metricSupport === 'bounded') return ['point', 'scaled_beta']
  if (form.metricSupport === 'non_negative') return ['point', 'log_normal', 'beta', 'scaled_beta']
  return ['point', 'normal', 'log_normal', 'beta', 'scaled_beta']
})
const metricEditorSupport = computed(() => form.metricSupport === 'non_negative' ? 'non_negative' as const : 'real' as const)
const metricMinimum = computed(() => form.metricSupport === 'bounded' ? form.metricLower : undefined)
const metricMaximum = computed(() => form.metricSupport === 'bounded' ? form.metricUpper : undefined)

watch(() => [form.metricSupport, form.metricLower, form.metricUpper] as const, ([support, lower, upper]) => {
  if (support === 'bounded' && lower < upper) {
    metricCurrent.value = { type: 'point', value: (lower + upper) / 2 }
  } else if (support === 'non_negative') {
    metricCurrent.value = { type: 'point', value: 0 }
  }
})

function quantitySupport(): QuantitySupport {
  if (form.metricSupport === 'bounded') {
    return { type: 'bounded', lower: form.metricLower, upper: form.metricUpper }
  }
  return { type: form.metricSupport }
}

function estimate(id: string, distribution: Distribution): Estimate {
  return {
    id,
    revision: 0,
    distribution,
    source: { type: 'distribution' },
    provenance: form.provenance
      .split('\n')
      .map((value) => value.trim())
      .filter(Boolean),
  }
}

function payload(): NodePayload {
  switch (form.kind) {
    case 'outcome':
      return {
        kind: 'outcome',
        properties: {
          direction: form.direction,
          current: estimate('A', currentState.value),
          desired: null,
          evidence: [],
        },
      }
    case 'metric':
      return {
        kind: 'metric',
        properties: {
          unit: form.unit.trim(),
          dimension: metricDimension.value ?? undefined,
          aggregation: form.aggregation.trim() || null,
          support: quantitySupport(),
          operational_definition: form.operationalDefinition.trim(),
          reference_time: form.referenceTime.trim() || null,
          resolution_source: form.resolutionSource.trim() || null,
          current: form.metricHasEstimate ? estimate('A', metricCurrent.value) : null,
        },
      }
    case 'intervention':
      return {
        kind: 'intervention',
        properties: {
          costs: [],
          duration: estimate('A', duration.value),
          probability_of_success: estimate('B', successProbability.value),
          acceptance_criteria: form.acceptanceCriteria
            .split('\n')
            .map((value) => value.trim())
            .filter(Boolean),
        },
      }
    default:
      return {
        kind: 'factor',
        properties: {
          current: estimate('A', currentState.value),
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
  if (!title || !name || (form.kind === 'metric' && !metricDimension.value)) return
  emit('submit', { name, title, payload: payload() })
}

async function next() {
  if (!identityValid.value) return
  step.value = 2
  await nextTick()
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="dialog-backdrop" @click.self="emit('close')">
      <form class="dialog node-dialog" aria-labelledby="create-node-title" @submit.prevent="submit">
        <header>
          <div>
            <span class="eyebrow">Graph element · step {{ step }} of 2</span>
            <h2 id="create-node-title">{{ step === 1 ? 'Add node' : setupTitle }}</h2>
          </div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>

        <div class="wizard-progress" aria-label="Node creation progress">
          <span :class="{ active: step === 1, complete: step === 2 }"><Check v-if="step === 2" :size="12" />1 <small>Identity</small></span>
          <span :class="{ active: step === 2 }">2 <small>Setup</small></span>
        </div>

        <template v-if="step === 1">
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
        </template>

        <template v-else>
          <section v-if="form.kind === 'outcome' || form.kind === 'factor'" class="wizard-setup">
            <div class="readiness-callout required"><strong>Current state</strong><span>Required for scenario propagation on a normalized 0–1 scale.</span></div>
            <DistributionEditor
              v-model="currentState"
              :families="['point', 'beta']"
              support="probability"
              point-label="Current state on [0, 1]"
            />
            <label>Provenance<textarea v-model="form.provenance" rows="3" placeholder="One assumption or source per line"></textarea></label>
          </section>
          <section v-else-if="form.kind === 'intervention'" class="wizard-setup">
            <div class="readiness-callout recommended"><strong>Success probability</strong><span>Used to sample whether modeled changes occur.</span></div>
            <DistributionEditor
              v-model="successProbability"
              :families="['point', 'beta', 'scaled_beta']"
              support="probability"
              point-label="Success probability"
            />
            <div class="readiness-callout recommended"><strong>Duration</strong><span>Planning periods before modeled changes begin.</span></div>
            <DistributionEditor
              v-model="duration"
              :families="['point', 'log_normal', 'beta', 'scaled_beta']"
              support="non_negative"
              point-label="Planning periods"
            />
            <label>Acceptance criteria<textarea v-model="form.acceptanceCriteria" rows="3" placeholder="One verifiable condition per line"></textarea></label>
            <label>Provenance<textarea v-model="form.provenance" rows="3" placeholder="One assumption or source per line"></textarea></label>
          </section>
          <section v-else class="wizard-setup">
            <div class="readiness-callout ready"><strong>{{ form.unit }}</strong><span>Measurement unit</span></div>
            <label>Aggregation<input v-model="form.aggregation" placeholder="Weekly median, rolling average, latest reading" /></label>
            <label>Support<select v-model="form.metricSupport"><option value="real">Any real value</option><option value="non_negative">Zero or greater</option><option value="bounded">Bounded interval</option></select></label>
            <div v-if="form.metricSupport === 'bounded'" class="field-grid">
              <label>Minimum<input v-model.number="form.metricLower" type="number" step="any" required /></label>
              <label>Maximum<input v-model.number="form.metricUpper" type="number" step="any" required /></label>
            </div>
            <p v-if="!metricBoundsValid" class="form-error">The maximum must be greater than the minimum.</p>
            <label>Operational definition<textarea v-model="form.operationalDefinition" rows="3" placeholder="Exactly what is measured, over which population, and how it is calculated"></textarea></label>
            <div class="field-grid">
              <label>Reference time<input v-model="form.referenceTime" placeholder="2026 Q4, next 30 days, current" /></label>
              <label>Resolution source<input v-model="form.resolutionSource" placeholder="Dashboard, report, query, or authority" /></label>
            </div>
            <label class="checkbox-label"><input v-model="form.metricHasEstimate" type="checkbox" /> Add a current estimate</label>
            <DistributionEditor
              v-if="form.metricHasEstimate"
              v-model="metricCurrent"
              :families="metricFamilies"
              :support="metricEditorSupport"
              :minimum="metricMinimum"
              :maximum="metricMaximum"
              :point-label="`Value in ${form.unit}`"
            />
            <label v-if="form.metricHasEstimate">Provenance<textarea v-model="form.provenance" rows="3" placeholder="One assumption or source per line"></textarea></label>
          </section>
        </template>

        <footer>
          <button v-if="step === 1" type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button v-else type="button" class="secondary-button" @click="step = 1"><ArrowLeft :size="15" /> Back</button>
          <button v-if="step === 1" type="button" class="primary-button" :disabled="!identityValid" @click="next">Continue <ArrowRight :size="15" /></button>
          <button v-else type="submit" class="primary-button" :disabled="pending || !metricBoundsValid">
            {{ pending ? 'Adding…' : 'Add ready node' }}
          </button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>

<style scoped>
.node-dialog { width: min(620px, 100%); max-height: calc(100vh - 32px); overflow: auto; }
.kind-options { display: grid; grid-template-columns: repeat(4, 1fr); gap: 6px; }
.kind-options label { position: relative; }
.kind-options input { position: absolute; opacity: 0; }
.kind-options span { display: grid; place-items: center; min-height: 36px; border: 2px solid transparent; border-radius: 5px; color: #29312d; font-size: 10px; text-transform: capitalize; }
.kind-options input:checked + span { border-color: #26352e; }
.wizard-progress { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; margin: -4px 0 18px; }
.wizard-progress > span { min-height: 30px; display: flex; align-items: center; gap: 5px; padding: 0 9px; border-bottom: 2px solid var(--line); color: var(--muted); font: 10px 'IBM Plex Mono', monospace; }
.wizard-progress > span.active { border-bottom-color: var(--green); color: var(--green); }
.wizard-progress > span.complete { color: var(--green); }
.wizard-progress small { font: 9px 'Manrope', sans-serif; font-weight: 700; }
.wizard-setup { display: grid; gap: 14px; }
.wizard-setup :deep(.distribution-editor) { margin-top: -5px; padding-bottom: 14px; border-bottom: 1px solid var(--line); }
.wizard-setup :deep(.distribution-editor:last-of-type) { padding-bottom: 0; border-bottom: 0; }
.node-dialog > footer { position: sticky; z-index: 2; bottom: -20px; margin: 18px -20px -20px; padding: 12px 20px 20px; border-top: 1px solid var(--line); background: var(--surface-strong); }
.readiness-callout { display: grid; grid-template-columns: minmax(0, .42fr) minmax(0, 1fr); gap: 10px; padding: 9px 10px; border-left: 3px solid #a8bfb2; background: #f3f8f4; }
.readiness-callout strong { font-size: 10px; }
.readiness-callout span { color: var(--muted); font-size: 9px; line-height: 1.4; }
.readiness-callout.required { border-left-color: #a83f31; background: #fff8f6; }
.readiness-callout.recommended { border-left-color: #9a6a12; background: #fff8e9; }

@media (max-width: 760px) {
  .node-dialog { max-height: calc(100svh - 24px); }
  .kind-options { grid-template-columns: 1fr 1fr; }
  .readiness-callout { grid-template-columns: 1fr; gap: 3px; }
}
</style>
