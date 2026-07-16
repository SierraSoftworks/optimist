<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { Trash2, X } from '@lucide/vue'
import type {
  Distribution,
  Estimate,
  GraphNode,
  InterventionEstimateSlot,
  SetInterventionEstimateInput,
} from '../api/types'

const props = defineProps<{
  open: boolean
  pending: boolean
  node: GraphNode | null
  slot: InterventionEstimateSlot | null
}>()
const emit = defineEmits<{
  close: []
  submit: [input: SetInterventionEstimateInput]
  remove: [estimate: Estimate]
}>()
const form = reactive({
  dimension: '',
  family: 'point' as 'point' | 'log_normal' | 'beta' | 'scaled_beta',
  value: 0,
  location: 0,
  scale: 0.25,
  alpha: 2,
  beta: 2,
  lower: 0,
  upper: 1,
  provenance: '',
})
const confirmRemove = ref(false)
const probability = computed(() => props.slot?.kind === 'probability_of_success')
const duplicateCost = computed(() =>
  props.node?.payload.kind === 'intervention' &&
  props.slot?.kind === 'cost' &&
  !props.slot.value &&
  props.node.payload.properties.costs.some(
    (cost) => cost.dimension === form.dimension.trim(),
  ),
)
const existing = computed(() => {
  if (props.node?.payload.kind !== 'intervention' || !props.slot) return null
  const slot = props.slot
  if (slot.kind === 'duration') return props.node.payload.properties.duration
  if (slot.kind === 'probability_of_success') {
    return props.node.payload.properties.probability_of_success
  }
  return props.node.payload.properties.costs.find(
    (cost) => cost.dimension === slot.value,
  )?.value ?? null
})
const title = computed(() => {
  if (props.slot?.kind === 'duration') return 'Duration estimate'
  if (props.slot?.kind === 'probability_of_success') return 'Success probability'
  return props.slot?.value ? `${props.slot.value} cost` : 'Add cost dimension'
})

watch(
  () => [props.open, props.node, props.slot] as const,
  ([open]) => {
    if (!open || !props.slot) return
    const distribution = existing.value?.distribution
    form.dimension = props.slot.kind === 'cost' ? props.slot.value : ''
    form.family = distribution?.type === 'log_normal' || distribution?.type === 'beta' || distribution?.type === 'scaled_beta'
      ? distribution.type
      : 'point'
    form.value = distribution?.value ?? (probability.value ? 0.5 : 0)
    form.location = distribution?.location ?? 0
    form.scale = distribution?.scale ?? 0.25
    form.alpha = distribution?.alpha ?? 2
    form.beta = distribution?.beta ?? 2
    form.lower = distribution?.lower ?? 0
    form.upper = distribution?.upper ?? 1
    form.provenance = existing.value?.provenance?.join('\n') ?? ''
    confirmRemove.value = false
  },
)

function distribution(): Distribution {
  switch (form.family) {
    case 'log_normal':
      return { type: 'log_normal', location: form.location, scale: form.scale }
    case 'beta':
      return { type: 'beta', alpha: form.alpha, beta: form.beta }
    case 'scaled_beta':
      return {
        type: 'scaled_beta',
        alpha: form.alpha,
        beta: form.beta,
        lower: form.lower,
        upper: form.upper,
      }
    default:
      return { type: 'point', value: form.value }
  }
}

function submit() {
  if (!props.slot) return
  const slot = props.slot.kind === 'cost'
    ? { kind: 'cost' as const, value: form.dimension.trim() }
    : props.slot
  if ((slot.kind === 'cost' && !slot.value) || duplicateCost.value) return
  emit('submit', {
    slot,
    distribution: distribution(),
    provenance: form.provenance
      .split('\n')
      .map((value) => value.trim())
      .filter(Boolean),
  })
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open && node && slot" class="dialog-backdrop" @click.self="emit('close')">
      <form class="dialog estimate-dialog" aria-labelledby="edit-intervention-estimate-title" @submit.prevent="submit">
        <header>
          <div>
            <span class="eyebrow">{{ node.title }}</span>
            <h2 id="edit-intervention-estimate-title">{{ title }}</h2>
          </div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>
        <label v-if="slot.kind === 'cost'">Dimension<input v-model="form.dimension" :readonly="Boolean(existing)" placeholder="usd, engineer_days, risk" required /></label>
        <p v-if="duplicateCost" class="form-error">This cost dimension already exists. Edit it from the inspector.</p>
        <label>
          Distribution
          <select v-model="form.family">
            <option value="point">Point</option>
            <option v-if="!probability" value="log_normal">LogNormal</option>
            <option value="beta">Beta</option>
            <option value="scaled_beta">Scaled Beta</option>
          </select>
        </label>
        <label v-if="form.family === 'point'">Value<input v-model.number="form.value" type="number" min="0" :max="probability ? 1 : undefined" step="any" required /></label>
        <div v-else-if="form.family === 'log_normal'" class="field-grid distribution-fields">
          <label>Log location<input v-model.number="form.location" type="number" step="any" required /></label>
          <label>Log scale<input v-model.number="form.scale" type="number" min="0.000001" step="any" required /></label>
        </div>
        <div v-else class="field-grid distribution-fields">
          <label>Alpha<input v-model.number="form.alpha" type="number" min="0.000001" step="any" required /></label>
          <label>Beta<input v-model.number="form.beta" type="number" min="0.000001" step="any" required /></label>
          <template v-if="form.family === 'scaled_beta'">
            <label>Lower bound<input v-model.number="form.lower" type="number" min="0" :max="probability ? 1 : undefined" step="any" required /></label>
            <label>Upper bound<input v-model.number="form.upper" type="number" min="0" :max="probability ? 1 : undefined" step="any" required /></label>
          </template>
        </div>
        <label>Provenance<textarea v-model="form.provenance" rows="4" placeholder="One source or elicitation note per line"></textarea></label>
        <div v-if="confirmRemove" class="replace-warning">
          <Trash2 :size="18" />
          <div><strong>Remove this estimate?</strong><span>The slot will return to its unset state.</span></div>
        </div>
        <footer>
          <button v-if="existing" type="button" class="danger-button" :disabled="pending" @click="confirmRemove ? emit('remove', existing) : (confirmRemove = true)"><Trash2 :size="14" /> {{ confirmRemove ? 'Confirm remove' : 'Remove' }}</button>
          <span class="footer-spacer"></span>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending || duplicateCost || (slot.kind === 'cost' && !form.dimension.trim())">{{ pending ? 'Saving…' : existing ? 'Replace estimate' : 'Set estimate' }}</button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>
