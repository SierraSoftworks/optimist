<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Trash2, X } from '@lucide/vue'
import type {
  EdgeEstimateSlot,
  Estimate,
  EstimateSourceInput,
  GraphEdge,
  SetEdgeEstimateInput,
  Unit,
} from '../api/types'
import { defaultSquiggleSourceInput } from '../domain/squiggleEstimate'
import EstimateSourceEditor from './EstimateSourceEditor.vue'

const props = defineProps<{
  open: boolean
  pending: boolean
  projectId: string | null
  edge: GraphEdge | null
  slot: EdgeEstimateSlot | null
}>()
const emit = defineEmits<{
  close: []
  submit: [input: SetEdgeEstimateInput]
  remove: [estimate: Estimate]
}>()
const source = ref<EstimateSourceInput>(defaultSquiggleSourceInput('signed', {}))
const sourceValid = ref(true)
const confirmRemove = ref(false)
const signed = computed(() => props.slot?.kind === 'effect' || props.slot?.kind === 'degree')
const existing = computed(() => {
  if (!props.edge || !props.slot) return null
  if (props.edge.payload.kind === 'contributes' || props.edge.payload.kind === 'changes') {
    if (props.slot.kind === 'effect') return props.edge.payload.properties.effect ?? null
    if (props.slot.kind === 'response') return props.edge.payload.properties.response?.destination_change ?? null
    if (props.slot.kind === 'lag') return props.edge.payload.properties.lag
  }
  if (props.edge.payload.kind === 'blocks' && props.slot.kind === 'degree') {
    return props.edge.payload.properties.degree
  }
  return null
})
const title = computed(() => {
  if (props.slot?.kind === 'effect') return 'Causal effect'
  if (props.slot?.kind === 'response') return 'Destination response'
  if (props.slot?.kind === 'degree') return 'Blocking degree'
  return 'Causal lag'
})
const expectedUnit = computed<Unit>(() => {
  if (props.slot?.kind === 'lag') return { duration: 1 }
  if (
    props.slot?.kind === 'response' &&
    props.edge?.payload.kind === 'contributes' &&
    props.edge.payload.properties.response
  ) {
    return props.edge.payload.properties.response.destination_unit
  }
  return {} as Unit
})

watch(
  () => [props.open, props.edge, props.slot] as const,
  ([open]) => {
    if (!open || !props.slot) return
    source.value = existing.value?.source?.type === 'fermi'
      ? { type: 'fermi', definition: existing.value.source.definition }
      : existing.value?.source?.type === 'squiggle'
        ? { type: 'squiggle', definition: existing.value.source.definition }
        : existing.value
          ? { type: 'distribution', distribution: existing.value.distribution }
          : defaultSquiggleSourceInput(
              signed.value ? 'signed' : props.slot.kind === 'response' ? 'real' : 'non_negative',
              expectedUnit.value,
            )
    sourceValid.value = true
    confirmRemove.value = false
  },
)

function submit() {
  if (!props.slot) return
  emit('submit', {
    slot: props.slot,
    source: source.value,
    provenance: existing.value?.provenance ?? [],
    uncertainty: existing.value?.uncertainty,
  })
}

</script>

<template>
  <Teleport to="body">
    <div v-if="open && edge && slot" class="dialog-backdrop" @click.self="emit('close')">
      <form class="dialog estimate-dialog" aria-labelledby="edit-edge-estimate-title" @submit.prevent="submit">
        <header>
          <div>
            <span class="eyebrow">{{ edge.source }} · {{ edge.payload.kind.replaceAll('_', ' ') }} · {{ edge.destination }}</span>
            <h2 id="edit-edge-estimate-title">{{ title }}</h2>
          </div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>
        <EstimateSourceEditor
          v-model="source"
          :existing="existing"
          :support="signed ? 'signed' : slot.kind === 'response' ? 'real' : 'non_negative'"
          :project-id="projectId"
          :expected-unit="expectedUnit"
          @validity="sourceValid = $event"
        />
        <div v-if="confirmRemove" class="replace-warning">
          <Trash2 :size="18" />
          <div><strong>Remove this lag?</strong><span>The relationship effect and mechanism remain unchanged.</span></div>
        </div>
        <footer>
          <button v-if="slot.kind === 'lag' && existing" type="button" class="danger-button" :disabled="pending" @click="confirmRemove ? emit('remove', existing) : (confirmRemove = true)"><Trash2 :size="14" /> {{ confirmRemove ? 'Confirm remove' : 'Remove lag' }}</button>
          <span class="footer-spacer"></span>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending || !sourceValid">{{ pending ? 'Saving…' : existing ? 'Replace estimate' : 'Set estimate' }}</button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>
