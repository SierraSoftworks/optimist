<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { Trash2, X } from '@lucide/vue'
import type {
  Estimate,
  EstimateSourceInput,
  GraphNode,
  InterventionEstimateSlot,
  SetInterventionEstimateInput,
} from '../api/types'
import { defaultSquiggleSourceInput } from '../domain/squiggleEstimate'
import EstimateSourceEditor from './EstimateSourceEditor.vue'

const props = defineProps<{
  open: boolean
  pending: boolean
  projectId: string | null
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
  provenance: '',
})
const source = ref<EstimateSourceInput>(defaultSquiggleSourceInput('non_negative', {}))
const sourceValid = ref(true)
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
const expectedUnit = computed(() => {
  if (props.slot?.kind === 'duration') return { duration: 1 }
  if (props.slot?.kind === 'cost' && form.dimension.trim()) return { [form.dimension.trim()]: 1 }
  return {}
})

watch(
  () => [props.open, props.node, props.slot] as const,
  ([open]) => {
    if (!open || !props.slot) return
    const currentDistribution = existing.value?.distribution
    form.dimension = props.slot.kind === 'cost' ? props.slot.value : ''
    source.value = existing.value?.source?.type === 'fermi'
      ? { type: 'fermi', definition: existing.value.source.definition }
      : existing.value?.source?.type === 'squiggle'
        ? { type: 'squiggle', definition: existing.value.source.definition }
        : currentDistribution
          ? { type: 'distribution', distribution: currentDistribution }
          : defaultSquiggleSourceInput(probability.value ? 'probability' : 'non_negative', expectedUnit.value)
    sourceValid.value = true
    form.provenance = existing.value?.provenance?.join('\n') ?? ''
    confirmRemove.value = false
  },
)

function submit() {
  if (!props.slot) return
  const slot = props.slot.kind === 'cost'
    ? { kind: 'cost' as const, value: form.dimension.trim() }
    : props.slot
  if ((slot.kind === 'cost' && !slot.value) || duplicateCost.value) return
  emit('submit', {
    slot,
    source: source.value,
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
        <EstimateSourceEditor
          v-model="source"
          :existing="existing"
          :support="probability ? 'probability' : 'non_negative'"
          :project-id="projectId"
          :expected-unit="expectedUnit"
          @validity="sourceValid = $event"
        />
        <label>Provenance<textarea v-model="form.provenance" rows="4" placeholder="One source or elicitation note per line"></textarea></label>
        <div v-if="confirmRemove" class="replace-warning">
          <Trash2 :size="18" />
          <div><strong>Remove this estimate?</strong><span>The slot will return to its unset state.</span></div>
        </div>
        <footer>
          <button v-if="existing" type="button" class="danger-button" :disabled="pending" @click="confirmRemove ? emit('remove', existing) : (confirmRemove = true)"><Trash2 :size="14" /> {{ confirmRemove ? 'Confirm remove' : 'Remove' }}</button>
          <span class="footer-spacer"></span>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending || !sourceValid || duplicateCost || (slot.kind === 'cost' && !form.dimension.trim())">{{ pending ? 'Saving…' : existing ? 'Replace estimate' : 'Set estimate' }}</button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>
