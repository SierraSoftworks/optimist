<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from 'vue'
import { ArrowLeft, ArrowRight, Check, X } from '@lucide/vue'
import type {
  CreateNodeInput,
  Distribution,
  Estimate,
  NodeKind,
  NodePayload,
} from '../api/types'
import DistributionEditor from './DistributionEditor.vue'

const props = defineProps<{ open: boolean; pending: boolean }>()
const emit = defineEmits<{ close: []; submit: [input: CreateNodeInput] }>()
const titleInput = ref<HTMLInputElement>()
const step = ref<1 | 2>(1)
const currentState = ref<Distribution>({ type: 'beta', alpha: 2, beta: 2 })
const successProbability = ref<Distribution>({ type: 'beta', alpha: 4, beta: 2 })
const duration = ref<Distribution>({ type: 'log_normal', location: Math.log(4), scale: 0.35 })
const form = reactive({
  kind: 'factor' as NodeKind,
  title: '',
  name: '',
  direction: 'maximize' as 'maximize' | 'minimize' | 'target_range',
  unit: '',
  aggregation: '',
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
      controllable: false,
      provenance: '',
      acceptanceCriteria: '',
    })
    currentState.value = { type: 'beta', alpha: 2, beta: 2 }
    successProbability.value = { type: 'beta', alpha: 4, beta: 2 }
    duration.value = { type: 'log_normal', location: Math.log(4), scale: 0.35 }
    step.value = 1
    await nextTick()
    titleInput.value?.focus()
  },
)

const identityValid = computed(() =>
  Boolean(
    form.title.trim() &&
    (form.name.trim() || generatedName.value) &&
    (form.kind !== 'metric' || form.unit.trim()),
  ),
)

const setupTitle = computed(() => {
  if (form.kind === 'metric') return 'Measurement setup'
  if (form.kind === 'intervention') return 'Planning estimates'
  return 'Simulation baseline'
})

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
          aggregation: form.aggregation.trim() || null,
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
  if (!title || !name || (form.kind === 'metric' && !form.unit.trim())) return
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
          </section>
        </template>

        <footer>
          <button v-if="step === 1" type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button v-else type="button" class="secondary-button" @click="step = 1"><ArrowLeft :size="15" /> Back</button>
          <button v-if="step === 1" type="button" class="primary-button" :disabled="!identityValid" @click="next">Continue <ArrowRight :size="15" /></button>
          <button v-else type="submit" class="primary-button" :disabled="pending">
            {{ pending ? 'Adding…' : 'Add ready node' }}
          </button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>
