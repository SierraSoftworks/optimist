<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { X } from '@lucide/vue'
import type { EstimateAddress, EstimateSourceInput, EstimateSupport, GraphEdge, GraphNode, ProjectDependenceModel, QuantitySupport, SetStateEstimateInput, StateEstimateSlot, Unit } from '../api/types'
import type { CatalogueEntry } from '../domain/estimateCatalogue'
import { defaultSquiggleSourceInput } from '../domain/squiggleEstimate'
import { formatUnitExpression } from '../domain/unitExpression'
import EstimateSourceEditor from './EstimateSourceEditor.vue'
import SharedQuantityEditor from './SharedQuantityEditor.vue'

const props = defineProps<{
  open: boolean
  pending: boolean
  node: GraphNode | null
  projectId: string | null
  edges: GraphEdge[]
  catalogue: CatalogueEntry[]
  dependence: ProjectDependenceModel | null
}>()
const emit = defineEmits<{
  close: []
  submit: [input: SetStateEstimateInput]
  share: [input: { address: EstimateAddress; partner: CatalogueEntry }]
  unshare: [address: EstimateAddress]
}>()
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

/// An estimate can only be coupled once it exists and therefore has an ID.
const address = computed<EstimateAddress | null>(() =>
  props.projectId && props.node && existing.value
    ? {
        project: props.projectId,
        owner: { kind: 'node', id: props.node.id },
        estimate: existing.value.id,
      }
    : null,
)
const unitText = computed(() => formatUnitExpression(expectedUnit.value))
const authoredSource = computed(() => source.value.definition.source)

/// Adopting the partner's source is what makes the two marginals identical.
function share(partner: CatalogueEntry) {
  if (!address.value) return
  source.value = {
    type: 'squiggle',
    definition: { ...source.value.definition, source: partner.source },
  }
  emit('share', { address: address.value, partner })
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
        <SharedQuantityEditor
          v-if="canAuthor"
          :address="address"
          :unit="unitText"
          :source="authoredSource"
          :catalogue="catalogue"
          :dependence="dependence"
          :pending="pending"
          @share="share"
          @unshare="address && emit('unshare', address)"
        />
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
