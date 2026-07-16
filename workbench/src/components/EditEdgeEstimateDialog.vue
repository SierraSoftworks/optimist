<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { Trash2, X } from '@lucide/vue'
import type {
  Distribution,
  EdgeEstimateSlot,
  Estimate,
  GraphEdge,
  SetEdgeEstimateInput,
  Unit,
} from '../api/types'
import DistributionEditor from './DistributionEditor.vue'

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
const form = reactive({
  provenance: '',
})
const distribution = ref<Distribution>({ type: 'point', value: 0 })
const confirmRemove = ref(false)
const signed = computed(() => props.slot?.kind === 'effect' || props.slot?.kind === 'degree')
const families = computed<Array<Distribution['type']>>(() =>
  signed.value
    ? ['point', 'beta', 'scaled_beta']
    : ['point', 'log_normal', 'beta', 'scaled_beta'],
)
const existing = computed(() => {
  if (!props.edge || !props.slot) return null
  if (props.edge.payload.kind === 'contributes' || props.edge.payload.kind === 'changes') {
    if (props.slot.kind === 'effect') return props.edge.payload.properties.effect
    if (props.slot.kind === 'lag') return props.edge.payload.properties.lag
  }
  if (props.edge.payload.kind === 'blocks' && props.slot.kind === 'degree') {
    return props.edge.payload.properties.degree
  }
  return null
})
const title = computed(() => {
  if (props.slot?.kind === 'effect') return 'Causal effect'
  if (props.slot?.kind === 'degree') return 'Blocking degree'
  return 'Causal lag'
})
const expectedUnit = computed<Unit>(() => {
  if (props.slot?.kind === 'lag') return { duration: 1 }
  return {} as Unit
})

watch(
  () => [props.open, props.edge, props.slot] as const,
  ([open]) => {
    if (!open || !props.slot) return
    distribution.value = existing.value?.distribution ?? { type: 'point', value: 0 }
    form.provenance = existing.value?.provenance?.join('\n') ?? ''
    confirmRemove.value = false
  },
)

function submit() {
  if (!props.slot) return
  emit('submit', {
    slot: props.slot,
    distribution: distribution.value,
    provenance: form.provenance
      .split('\n')
      .map((value) => value.trim())
      .filter(Boolean),
  })
}

function appendFermiProvenance(value: string) {
  form.provenance = [form.provenance.trim(), value].filter(Boolean).join('\n')
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
        <DistributionEditor
          v-model="distribution"
          :families="families"
          :support="signed ? 'signed' : 'non_negative'"
          :project-id="projectId"
          :expected-unit="expectedUnit"
          @fermi-provenance="appendFermiProvenance"
        />
        <label>Provenance<textarea v-model="form.provenance" rows="4" placeholder="One source or elicitation note per line"></textarea></label>
        <div v-if="confirmRemove" class="replace-warning">
          <Trash2 :size="18" />
          <div><strong>Remove this lag?</strong><span>The relationship effect and mechanism remain unchanged.</span></div>
        </div>
        <footer>
          <button v-if="slot.kind === 'lag' && existing" type="button" class="danger-button" :disabled="pending" @click="confirmRemove ? emit('remove', existing) : (confirmRemove = true)"><Trash2 :size="14" /> {{ confirmRemove ? 'Confirm remove' : 'Remove lag' }}</button>
          <span class="footer-spacer"></span>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending">{{ pending ? 'Saving…' : existing ? 'Replace estimate' : 'Set estimate' }}</button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>
