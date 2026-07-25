<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from 'vue'
import { X } from '@lucide/vue'
import type {
  CreateNodeInput,
  NodeKind,
  NodePayload,
  QuantitySupport,
} from '../api/types'
import { parseUnitExpression } from '../domain/unitExpression'

const props = defineProps<{ open: boolean; pending: boolean }>()
const emit = defineEmits<{ close: []; submit: [input: CreateNodeInput] }>()
const titleInput = ref<HTMLInputElement>()
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
      aggregation: '',
      metricSupport: 'real',
      metricLower: 0,
      metricUpper: 1,
      operationalDefinition: '',
      referenceTime: '',
      resolutionSource: '',
      controllable: false,
    })
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

const metricBoundsValid = computed(() =>
  form.metricSupport !== 'bounded' || (
    Number.isFinite(form.metricLower) &&
    Number.isFinite(form.metricUpper) &&
    form.metricLower < form.metricUpper
  ),
)

function quantitySupport(): QuantitySupport {
  if (form.metricSupport === 'bounded') {
    return { type: 'bounded', lower: form.metricLower, upper: form.metricUpper }
  }
  return { type: form.metricSupport }
}

function payload(): NodePayload {
  switch (form.kind) {
    case 'outcome':
      return {
        kind: 'outcome',
        properties: {
          direction: form.direction,
          evidence: [],
        },
      }
    case 'metric':
      return {
        kind: 'metric',
        properties: {
          quantity: {
            unit: form.unit.trim(),
            dimension: metricDimension.value ?? undefined,
            aggregation: form.aggregation.trim() || null,
            support: quantitySupport(),
            operational_definition: form.operationalDefinition.trim(),
            reference_time: form.referenceTime.trim() || null,
            resolution_source: form.resolutionSource.trim() || null,
          },
          current: null,
        },
      }
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
          controllable: form.controllable,
          evidence: [],
        },
      }
  }
}

function submit() {
  const title = form.title.trim()
  const name = form.name.trim() || generatedName.value
  if (!title || !name || (form.kind === 'metric' && (!metricDimension.value || !metricBoundsValid.value))) return
  emit('submit', { name, title, payload: payload() })
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="dialog-backdrop" @pointerdown.self="emit('close')">
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
        <label v-if="form.kind === 'factor'" class="checkbox-label">
          <input v-model="form.controllable" type="checkbox" />
          Directly controllable
        </label>

        <section v-if="form.kind === 'metric'" class="node-setup">
            <label>Unit<input v-model="form.unit" placeholder="minutes" required /></label>
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
        </section>

        <footer>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending || !identityValid || !metricBoundsValid">
            {{ pending ? 'Adding…' : 'Add node' }}
          </button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>

<style scoped>
.node-dialog { width: min(720px, 100%); }
.kind-options { display: grid; grid-template-columns: repeat(4, 1fr); gap: 6px; }
.kind-options label { position: relative; }
.kind-options input { position: absolute; opacity: 0; }
.kind-options span { display: grid; place-items: center; min-height: 44px; border: 2px solid transparent; border-radius: 5px; color: #29312d; font-size: var(--text-md); text-transform: capitalize; }
.kind-options input:checked + span { border-color: #26352e; }
.node-setup { display: grid; gap: 18px; margin-top: 20px; padding-top: 20px; border-top: 1px solid var(--line); }

@media (max-width: 760px) {
  .kind-options { grid-template-columns: 1fr 1fr; }
}
</style>
