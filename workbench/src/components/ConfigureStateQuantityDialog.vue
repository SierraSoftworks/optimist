<script setup lang="ts">
import { computed, reactive, watch } from 'vue'
import { X } from '@lucide/vue'
import type { GraphNode, QuantitySupport, SetNodeQuantityStateInput } from '../api/types'
import { parseUnitExpression } from '../domain/unitExpression'

const props = defineProps<{ open: boolean; pending: boolean; node: GraphNode | null }>()
const emit = defineEmits<{ close: []; submit: [input: SetNodeQuantityStateInput] }>()
const form = reactive({
  unit: '',
  aggregation: '',
  support: 'real' as 'real' | 'non_negative' | 'bounded',
  lower: 0,
  upper: 1,
  operationalDefinition: '',
  referenceTime: '',
  resolutionSource: '',
  stateZero: 0,
  stateOne: 1,
})

watch(
  () => props.open,
  (open) => {
    if (!open) return
    Object.assign(form, {
      unit: '', aggregation: '', support: 'real', lower: 0, upper: 1,
      operationalDefinition: '', referenceTime: '', resolutionSource: '',
      stateZero: 0, stateOne: 1,
    })
  },
)

const dimension = computed(() => {
  try {
    return parseUnitExpression(form.unit)
  } catch {
    return null
  }
})
const boundsValid = computed(() =>
  form.support !== 'bounded' || (
    Number.isFinite(form.lower) && Number.isFinite(form.upper) && form.lower < form.upper
  ),
)
const hasLegacyState = computed(() => {
  if (props.node?.payload.kind !== 'factor' && props.node?.payload.kind !== 'outcome') return false
  return Boolean(props.node.payload.properties.current || props.node.payload.properties.desired)
})
const mappingValid = computed(() =>
  !hasLegacyState.value || (
    Number.isFinite(form.stateZero) &&
    Number.isFinite(form.stateOne) &&
    form.stateZero < form.stateOne
  ),
)
const valid = computed(() => Boolean(dimension.value && boundsValid.value && mappingValid.value))

function quantitySupport(): QuantitySupport {
  return form.support === 'bounded'
    ? { type: 'bounded', lower: form.lower, upper: form.upper }
    : { type: form.support }
}

function submit() {
  if (!valid.value || !dimension.value) return
  emit('submit', {
    quantity: {
      unit: form.unit.trim(),
      dimension: dimension.value,
      aggregation: form.aggregation.trim() || null,
      support: quantitySupport(),
      operational_definition: form.operationalDefinition.trim(),
      reference_time: form.referenceTime.trim() || null,
      resolution_source: form.resolutionSource.trim() || null,
    },
    legacy_mapping: hasLegacyState.value
      ? { state_zero: form.stateZero, state_one: form.stateOne }
      : undefined,
  })
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open && node" class="dialog-backdrop" @click.self="emit('close')">
      <form class="dialog" aria-labelledby="state-quantity-title" @submit.prevent="submit">
        <header>
          <div>
            <span class="eyebrow">{{ node.title }}</span>
            <h2 id="state-quantity-title">Configure native state</h2>
          </div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>
        <div class="field-grid">
          <label>Unit<input v-model="form.unit" placeholder="days, incidents/week" required /></label>
          <label>Aggregation<input v-model="form.aggregation" placeholder="p95 weekly" /></label>
        </div>
        <label>Support
          <select v-model="form.support">
            <option value="real">Any real value</option>
            <option value="non_negative">Zero or greater</option>
            <option value="bounded">Bounded interval</option>
          </select>
        </label>
        <div v-if="form.support === 'bounded'" class="field-grid">
          <label>Lower bound<input v-model.number="form.lower" type="number" required /></label>
          <label>Upper bound<input v-model.number="form.upper" type="number" required /></label>
        </div>
        <fieldset v-if="hasLegacyState" class="legacy-mapping">
          <legend>Convert existing standardized state</legend>
          <div class="field-grid">
            <label>Native value at state 0<input v-model.number="form.stateZero" type="number" step="any" required /></label>
            <label>Native value at state 1<input v-model.number="form.stateOne" type="number" step="any" required /></label>
          </div>
          <p v-if="!mappingValid" class="form-error">State 1 must be greater than state 0.</p>
        </fieldset>
        <label>Operational definition<textarea v-model="form.operationalDefinition" rows="3" placeholder="Exactly what this state measures"></textarea></label>
        <div class="field-grid">
          <label>Reference time<input v-model="form.referenceTime" placeholder="2026-Q4 or current" /></label>
          <label>Resolution source<input v-model="form.resolutionSource" placeholder="Dashboard, query, or authority" /></label>
        </div>
        <p class="muted">Native state requires unit-aware linear responses on causal relationships. This change is available before state estimates or normalized relationships are added.</p>
        <footer>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending || !valid">{{ pending ? 'Saving…' : 'Use native state' }}</button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>

<style scoped>
.legacy-mapping { margin-top: 20px !important; padding-top: 18px !important; border-top: 1px solid var(--line) !important; }
</style>
