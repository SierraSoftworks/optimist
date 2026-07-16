<script setup lang="ts">
import { computed, reactive, watch } from 'vue'
import { X } from '@lucide/vue'
import type { CreateEdgeInput, EdgeKind, GraphNode } from '../api/types'
import { destinationsFor, edgeKinds, edgePayload, sourcesFor } from '../domain/edgeAuthoring'

const props = defineProps<{ open: boolean; pending: boolean; nodes: GraphNode[] }>()
const emit = defineEmits<{ close: []; submit: [input: CreateEdgeInput] }>()
const form = reactive({
  source: '', destination: '', kind: 'contributes' as EdgeKind, effect: 0.5,
  lagEnabled: false, lag: 0, mechanism: '', evidence: '',
  polarity: 'higher_is_better' as 'higher_is_better' | 'lower_is_better' | 'target_range',
  hard: true, thresholdEnabled: false, threshold: 0.5,
})

const validSources = computed(() => sourcesFor(form.kind, props.nodes))
const source = computed(() => props.nodes.find((node) => node.id === form.source))
const validDestinations = computed(() => destinationsFor(form.kind, source.value, props.nodes))
const causal = computed(() => form.kind === 'contributes' || form.kind === 'changes')

watch(() => props.open, (open) => {
  if (!open) return
  Object.assign(form, {
    source: '', destination: '', kind: 'contributes', effect: 0.5,
    lagEnabled: false, lag: 0, mechanism: '', evidence: '',
    polarity: 'higher_is_better', hard: true, thresholdEnabled: false, threshold: 0.5,
  })
})

watch([() => form.source, () => form.kind], () => {
  if (!validSources.value.some((node) => node.id === form.source)) form.source = ''
  if (!validDestinations.value.some((node) => node.id === form.destination)) form.destination = ''
})

function submit() {
  if (!form.source || !form.destination) return
  emit('submit', {
    source: form.source,
    destination: form.destination,
    payload: edgePayload({
      kind: form.kind,
      effect: form.effect,
      lag: form.lagEnabled ? form.lag : null,
      mechanism: form.mechanism,
      evidence: form.evidence,
      polarity: form.polarity,
      hard: form.hard,
      threshold: form.thresholdEnabled ? form.threshold : null,
    }),
  })
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="dialog-backdrop" @click.self="emit('close')">
      <form class="dialog relationship-dialog" aria-labelledby="create-edge-title" @submit.prevent="submit">
        <header>
          <div><span class="eyebrow">Graph structure</span><h2 id="create-edge-title">Add relationship</h2></div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>
        <label>Relationship<select v-model="form.kind"><option v-for="item in edgeKinds" :key="item.kind" :value="item.kind">{{ item.label }}</option></select></label>
        <div class="field-grid relationship-fields">
          <label>Source<select v-model="form.source" required><option value="" disabled>Select node</option><option v-for="node in validSources" :key="node.id" :value="node.id">{{ node.title }} · {{ node.id }}</option></select></label>
          <label>Destination<select v-model="form.destination" required><option value="" disabled>Select node</option><option v-for="node in validDestinations" :key="node.id" :value="node.id">{{ node.title }} · {{ node.id }}</option></select></label>
        </div>
        <label v-if="causal || form.kind === 'blocks'">{{ form.kind === 'blocks' ? 'Blocking degree' : 'Signed effect' }} on [-1, 1]<input v-model.number="form.effect" type="number" min="-1" max="1" step="0.05" required /></label>
        <template v-if="causal">
          <label class="checkbox-label"><input v-model="form.lagEnabled" type="checkbox" /> Include lag</label>
          <label v-if="form.lagEnabled">Lag in planning periods<input v-model.number="form.lag" type="number" min="0" step="0.1" required /></label>
          <label>Mechanism<textarea v-model="form.mechanism" rows="3" placeholder="How this influence operates"></textarea></label>
          <label>Evidence references<textarea v-model="form.evidence" rows="3" placeholder="One citation or evidence ID per line"></textarea></label>
        </template>
        <label v-if="form.kind === 'measures'">Measurement polarity<select v-model="form.polarity"><option value="higher_is_better">Higher is better</option><option value="lower_is_better">Lower is better</option><option value="target_range">Target range</option></select></label>
        <template v-if="form.kind === 'requires'">
          <label class="checkbox-label"><input v-model="form.hard" type="checkbox" /> Hard prerequisite</label>
          <label class="checkbox-label"><input v-model="form.thresholdEnabled" type="checkbox" /> Include satisfaction threshold</label>
          <label v-if="form.thresholdEnabled">Satisfaction threshold on [0, 1]<input v-model.number="form.threshold" type="number" min="0" max="1" step="0.05" required /></label>
        </template>
        <p v-if="validSources.length === 0" class="form-note">Add compatible endpoint node kinds for this relationship first.</p>
        <footer><button type="button" class="secondary-button" @click="emit('close')">Cancel</button><button type="submit" class="primary-button" :disabled="pending || !form.destination">{{ pending ? 'Adding…' : 'Add relationship' }}</button></footer>
      </form>
    </div>
  </Teleport>
</template>
