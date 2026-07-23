<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { X } from '@lucide/vue'
import type { CreateEdgeInput, EdgeKind, Estimate, GraphNode, SquiggleAssessmentResult } from '../api/types'
import { destinationsFor, edgeKinds, edgePayload, nodeUnit, sourcesFor } from '../domain/edgeAuthoring'
import { squiggleDefinition } from '../domain/squiggleEstimate'
import { formatUnitExpression } from '../domain/unitExpression'
import SquiggleEstimateEditor from './SquiggleEstimateEditor.vue'

const props = defineProps<{
  open: boolean
  pending: boolean
  projectId: string | null
  nodes: GraphNode[]
  initialSourceId?: string | null
  initialKind?: EdgeKind | null
}>()
const emit = defineEmits<{ close: []; submit: [input: CreateEdgeInput] }>()
const responseDefinition = ref(squiggleDefinition('pointMass(1)', {}))
const responseAssessment = ref<SquiggleAssessmentResult | null>(null)
const responseValid = ref(false)
const form = reactive({
  source: '', destination: '', kind: 'contributes' as EdgeKind, effect: 0.5,
  lagEnabled: false, lag: 0,
  polarity: 'higher_is_better' as 'higher_is_better' | 'lower_is_better' | 'target_range',
  hard: true, thresholdEnabled: false, threshold: 0.5,
  sourceChange: 1, destinationChange: 1,
})

const validSources = computed(() => sourcesFor(form.kind, props.nodes))
const source = computed(() => props.nodes.find((node) => node.id === form.source))
const validDestinations = computed(() => destinationsFor(form.kind, source.value, props.nodes))
const validKinds = computed(() => {
  const initialSource = props.nodes.find((node) => node.id === props.initialSourceId)
  if (!initialSource) return edgeKinds
  return edgeKinds.filter(({ kind }) => destinationsFor(kind, initialSource, props.nodes).length > 0)
})
const causal = computed(() => form.kind === 'contributes' || form.kind === 'changes')
const destination = computed(() => props.nodes.find((node) => node.id === form.destination))
const nativeCausal = causal
const sourceUnit = computed(() => source.value ? nodeUnit(source.value) : null)
const destinationUnit = computed(() => destination.value ? nodeUnit(destination.value) : null)
const nativeUnitsReady = computed(() => !nativeCausal.value || (sourceUnit.value !== null && destinationUnit.value !== null))

watch(() => props.open, (open) => {
  if (!open) return
  Object.assign(form, {
    source: props.initialSourceId ?? '', destination: '', kind: props.initialKind ?? 'contributes', effect: 0.5,
    lagEnabled: false, lag: 0,
    polarity: 'higher_is_better', hard: true, thresholdEnabled: false, threshold: 0.5,
    sourceChange: 1, destinationChange: 1,
  })
  resetResponse()
})

watch([() => form.source, () => form.kind], () => {
  if (!validSources.value.some((node) => node.id === form.source)) form.source = ''
  if (!validDestinations.value.some((node) => node.id === form.destination)) form.destination = ''
})

watch([() => form.destination, nativeCausal], resetResponse)

function submit() {
  if (!form.source || !form.destination) return
  const destinationEstimate = causal.value ? assessedResponse() : undefined
  if (causal.value && !destinationEstimate) return
  emit('submit', {
    source: form.source,
    destination: form.destination,
    payload: edgePayload({
      kind: form.kind,
      effect: form.effect,
      lag: form.lagEnabled ? form.lag : null,
      mechanism: '',
      evidence: '',
      polarity: form.polarity,
      hard: form.hard,
      threshold: form.thresholdEnabled ? form.threshold : null,
      source: source.value,
      destination: destination.value,
      sourceChange: form.sourceChange,
      destinationChange: form.destinationChange,
      destinationEstimate,
    }),
  })
}

function resetResponse() {
  responseAssessment.value = null
  responseValid.value = false
  responseDefinition.value = squiggleDefinition('pointMass(1)', destinationUnit.value ?? {})
}

function assessedResponse(): Estimate | undefined {
  if (!responseAssessment.value) return undefined
  return {
    id: 'A',
    revision: 0,
    distribution: responseAssessment.value.effective_distribution,
    source: {
      type: 'squiggle',
      definition: responseDefinition.value,
      assessment: responseAssessment.value.assessment,
    },
    provenance: [],
  }
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="dialog-backdrop" @click.self="emit('close')">
      <form class="dialog relationship-dialog" :class="{ 'native-relationship-dialog': nativeCausal }" aria-labelledby="create-edge-title" @submit.prevent="submit">
        <header>
          <div><span class="eyebrow">Graph structure</span><h2 id="create-edge-title">Add relationship</h2></div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>
        <label>Relationship<select v-model="form.kind"><option v-for="item in validKinds" :key="item.kind" :value="item.kind">{{ item.label }}</option></select></label>
        <div class="field-grid relationship-fields">
          <label>Source<select v-model="form.source" required :disabled="Boolean(initialSourceId)"><option value="" disabled>Select node</option><option v-for="node in validSources" :key="node.id" :value="node.id">{{ node.title }} · {{ node.id }}</option></select></label>
          <label>Destination<select v-model="form.destination" required><option value="" disabled>Select node</option><option v-for="node in validDestinations" :key="node.id" :value="node.id">{{ node.title }} · {{ node.id }}</option></select></label>
        </div>
        <label v-if="form.kind === 'blocks'">Blocking degree on [0, 1]<input v-model.number="form.effect" type="number" min="0" max="1" step="0.05" required /></label>
        <section v-if="nativeCausal" class="native-response">
          <header><strong>Counterfactual response</strong><span>Model destination movement for one source movement. This is a local response assumption, not causation inferred from correlation.</span></header>
          <label>{{ form.kind === 'changes' ? 'Intervention activation' : 'Source change' }} ({{ sourceUnit ? formatUnitExpression(sourceUnit) : 'unit unavailable' }})<input v-model.number="form.sourceChange" type="number" step="any" required /></label>
          <SquiggleEstimateEditor
            v-model="responseDefinition"
            :project-id="projectId"
            support="real"
            :expected-unit="destinationUnit ?? {}"
            @validity="responseValid = $event"
            @assessment="responseAssessment = $event"
          />
          <p v-if="!nativeUnitsReady" class="form-error">Both endpoints need canonical unit terms before this relationship can be created.</p>
        </section>
        <template v-if="causal">
          <label class="checkbox-label"><input v-model="form.lagEnabled" type="checkbox" /> Include lag</label>
          <label v-if="form.lagEnabled">Lag in planning periods<input v-model.number="form.lag" type="number" min="0" step="0.1" required /></label>
        </template>
        <label v-if="form.kind === 'measures'">Measurement polarity<select v-model="form.polarity"><option value="higher_is_better">Higher is better</option><option value="lower_is_better">Lower is better</option><option value="target_range">Target range</option></select></label>
        <template v-if="form.kind === 'requires'">
          <label class="checkbox-label"><input v-model="form.hard" type="checkbox" /> Hard prerequisite</label>
          <label class="checkbox-label"><input v-model="form.thresholdEnabled" type="checkbox" /> Include satisfaction threshold</label>
          <label v-if="form.thresholdEnabled">Satisfaction threshold on [0, 1]<input v-model.number="form.threshold" type="number" min="0" max="1" step="0.05" required /></label>
        </template>
        <p v-if="validSources.length === 0" class="form-note">Add compatible endpoint node kinds for this relationship first.</p>
        <footer><button type="button" class="secondary-button" @click="emit('close')">Cancel</button><button type="submit" class="primary-button" :disabled="pending || !form.destination || !nativeUnitsReady || (nativeCausal && (!responseValid || form.sourceChange === 0))">{{ pending ? 'Adding…' : 'Add relationship' }}</button></footer>
      </form>
    </div>
  </Teleport>
</template>

<style scoped>
.relationship-fields { margin-top: 14px; }
.relationship-dialog { width: min(680px, 100%); }
.native-relationship-dialog { width: min(1040px, 100%); }
.native-response { display: grid; gap: 10px; }
.native-response > header { display: grid; gap: 3px; padding-bottom: 8px; border-bottom: 1px solid var(--line); }
.native-response > header strong { font-size: 14px; }
.native-response > header span { color: var(--muted); font-size: 12px; line-height: 1.5; }
</style>
