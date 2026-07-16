<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { X } from '@lucide/vue'
import type { Distribution, GraphNode, SetStateEstimateInput, StateEstimateSlot } from '../api/types'
import DistributionEditor from './DistributionEditor.vue'

const props = defineProps<{ open: boolean; pending: boolean; node: GraphNode | null }>()
const emit = defineEmits<{ close: []; submit: [input: SetStateEstimateInput] }>()
const form = reactive({
  slot: 'current' as StateEstimateSlot,
  provenance: '',
})
const distribution = ref<Distribution>({ type: 'point', value: 0.5 })
const existing = computed(() => {
  if (props.node?.payload.kind !== 'factor' && props.node?.payload.kind !== 'outcome') return null
  return props.node.payload.properties[form.slot]
})

watch(
  () => [props.open, props.node, form.slot] as const,
  ([open]) => {
    if (!open) return
    const currentDistribution = existing.value?.distribution
    form.provenance = existing.value?.provenance?.join('\n') ?? ''
    if (currentDistribution?.type === 'beta' || currentDistribution?.type === 'point') {
      distribution.value = currentDistribution
    } else distribution.value = { type: 'point', value: 0.5 }
  },
)

function submit() {
  emit('submit', {
    slot: form.slot,
    distribution: distribution.value,
    provenance: form.provenance
      .split('\n')
      .map((value) => value.trim())
      .filter(Boolean),
  })
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open && node" class="dialog-backdrop" @click.self="emit('close')">
      <form class="dialog estimate-dialog" aria-labelledby="edit-estimate-title" @submit.prevent="submit">
        <header>
          <div>
            <span class="eyebrow">{{ node.title }}</span>
            <h2 id="edit-estimate-title">Set state estimate</h2>
          </div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>
        <label>
          State
          <select v-model="form.slot">
            <option value="current">Current</option>
            <option value="desired">Desired</option>
          </select>
        </label>
        <DistributionEditor v-model="distribution" :families="['point', 'beta']" support="probability" point-label="Value on [0, 1]" />
        <label>
          Provenance
          <textarea v-model="form.provenance" rows="4" placeholder="One source or elicitation note per line"></textarea>
        </label>
        <footer>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending">
            {{ pending ? 'Saving…' : existing ? 'Replace estimate' : 'Set estimate' }}
          </button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>
