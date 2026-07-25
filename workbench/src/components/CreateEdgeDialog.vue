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
  sourceId: string
  destinationId: string
  kind: EdgeKind
  sourceLocked: boolean
}>()
const emit = defineEmits<{
  close: []
  submit: [input: CreateEdgeInput]
  draftChange: [value: { sourceId: string; destinationId: string; kind: EdgeKind }]
}>()
const responseDefinition = ref(squiggleDefinition('pointMass(1)', {}))
const responseAssessment = ref<SquiggleAssessmentResult | null>(null)
const responseValid = ref(false)
const form = reactive({
  effect: 0.5, lagEnabled: false, lag: 0,
  polarity: 'higher_is_better' as 'higher_is_better' | 'lower_is_better' | 'target_range',
  hard: true, thresholdEnabled: false, threshold: 0.5,
})

const validSources = computed(() => sourcesFor(props.kind, props.nodes))
const source = computed(() => props.nodes.find((node) => node.id === props.sourceId))
const validDestinations = computed(() => destinationsFor(props.kind, source.value, props.nodes))
const validKinds = computed(() => {
  if (!props.sourceLocked || !source.value) return edgeKinds
  return edgeKinds.filter(({ kind }) => destinationsFor(kind, source.value, props.nodes).length > 0)
})
const causal = computed(() => props.kind === 'contributes' || props.kind === 'changes')
const destination = computed(() => props.nodes.find((node) => node.id === props.destinationId))
const nativeCausal = causal
const sourceUnit = computed(() => source.value ? nodeUnit(source.value) : null)
const destinationUnit = computed(() => destination.value ? nodeUnit(destination.value) : null)
const assessmentProjectId = computed(() => props.projectId)

watch(() => props.open, (open) => {
  if (!open) return
  Object.assign(form, {
    effect: 0.5, lagEnabled: false, lag: 0,
    polarity: 'higher_is_better', hard: true, thresholdEnabled: false, threshold: 0.5,
  })
  resetResponse()
})

watch([() => props.destinationId, nativeCausal], resetResponse)

function changeKind(event: Event) {
  const kind = (event.target as HTMLSelectElement).value as EdgeKind
  emit('draftChange', {
    kind,
    sourceId: sourcesFor(kind, props.nodes).some((node) => node.id === props.sourceId)
      ? props.sourceId
      : '',
    destinationId: '',
  })
}

function changeSource(sourceId: string) {
  emit('draftChange', {
    kind: props.kind,
    sourceId,
    destinationId: '',
  })
}

function changeDestination(destinationId: string) {
  emit('draftChange', {
    kind: props.kind,
    sourceId: props.sourceId,
    destinationId,
  })
}

function submit() {
  if (!source.value || !destination.value) return
  const responseEstimate = causal.value ? assessedResponse() : undefined
  if (causal.value && !responseEstimate) return
  emit('submit', {
    source: props.sourceId,
    destination: props.destinationId,
    payload: edgePayload({
      kind: props.kind,
      effect: form.effect,
      lag: form.lagEnabled ? form.lag : null,
      mechanism: '',
      evidence: '',
      polarity: form.polarity,
      hard: form.hard,
      threshold: form.thresholdEnabled ? form.threshold : null,
      source: source.value,
      destination: destination.value,
      responseEstimate,
    }),
  })
}

function resetResponse() {
  responseAssessment.value = null
  responseValid.value = false
  responseDefinition.value = squiggleDefinition('pointMass(1)', {})
}

function assessedResponse(): Estimate | undefined {
  if (!responseAssessment.value) return undefined
  return {
    id: 'A',
    revision: 0,
    source: {
      type: 'squiggle',
      definition: responseDefinition.value,
    },
    provenance: [],
  }
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="dialog-backdrop" @pointerdown.self="emit('close')">
      <form class="dialog relationship-dialog" :class="{ 'native-relationship-dialog': nativeCausal }" aria-labelledby="create-edge-title" @submit.prevent="submit">
        <header>
          <div><span class="eyebrow">Graph structure</span><h2 id="create-edge-title">Add relationship</h2></div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>
        <label>Relationship<select :value="kind" @change="changeKind"><option v-for="item in validKinds" :key="item.kind" :value="item.kind">{{ item.label }}</option></select></label>
        <div class="field-grid relationship-fields endpoint-fields">
          <fieldset class="node-picker">
            <legend>Source</legend>
            <div class="node-picker-options">
              <label v-for="node in validSources" :key="node.id">
                <input
                  type="radio"
                  name="relationship-source"
                  :value="node.id"
                  :checked="sourceId === node.id"
                  :disabled="sourceLocked"
                  @change="changeSource(node.id)"
                />
                <span class="node-picker-option">
                  <span class="kind-dot" :data-kind="node.payload.kind"></span>
                  <span><strong>{{ node.title }}</strong><small>{{ node.payload.kind }} · {{ node.id }}</small></span>
                </span>
              </label>
            </div>
            <p v-if="!validSources.length" class="form-note">No valid source nodes.</p>
          </fieldset>
          <fieldset class="node-picker">
            <legend>Target</legend>
            <div class="node-picker-options">
              <label v-for="node in validDestinations" :key="node.id">
                <input
                  type="radio"
                  name="relationship-destination"
                  :value="node.id"
                  :checked="destinationId === node.id"
                  @change="changeDestination(node.id)"
                />
                <span class="node-picker-option">
                  <span class="kind-dot" :data-kind="node.payload.kind"></span>
                  <span><strong>{{ node.title }}</strong><small>{{ node.payload.kind }} · {{ node.id }}</small></span>
                </span>
              </label>
            </div>
            <p v-if="sourceId && !validDestinations.length" class="form-note">No valid targets for this source.</p>
            <p v-else-if="!sourceId" class="form-note">Choose a source first.</p>
          </fieldset>
        </div>
        <label v-if="kind === 'blocks'">Blocking degree on [0, 1]<input v-model.number="form.effect" type="number" min="0" max="1" step="0.05" required /></label>
        <section v-if="nativeCausal" class="native-response">
          <header>
            <strong>{{ kind === 'changes' ? 'Intervention multiplier' : 'Proportional response' }}</strong>
            <span v-if="kind === 'changes'">The factor this intervention multiplies its target by while fully active. 0.1 cuts it to a tenth, 1 leaves it unchanged, 1.25 raises it by a quarter. This is a local response assumption, not causation inferred from correlation.</span>
            <span v-else>The elasticity of {{ destination?.title ?? 'the target' }} to {{ source?.title ?? 'the source' }}: doubling the source multiplies the target by 2 raised to this power. 1 is a plain product, 0 is no response, and negative values invert the direction. This is a local response assumption, not causation inferred from correlation.</span>
          </header>
          <p class="response-units">
            <span>{{ sourceUnit ? formatUnitExpression(sourceUnit) : 'no declared unit' }}</span>
            <span aria-hidden="true">→</span>
            <span>{{ destinationUnit ? formatUnitExpression(destinationUnit) : 'no declared unit' }}</span>
            <small>Ratios carry no unit, so the endpoints need not agree.</small>
          </p>
          <SquiggleEstimateEditor
            v-model="responseDefinition"
            :project-id="assessmentProjectId"
            support="real"
            :expected-unit="{}"
            @validity="responseValid = $event"
            @assessment="responseAssessment = $event"
          />
        </section>
        <template v-if="causal">
          <label class="checkbox-label"><input v-model="form.lagEnabled" type="checkbox" /> Include lag</label>
          <label v-if="form.lagEnabled">Lag in planning periods<input v-model.number="form.lag" type="number" min="0" step="0.1" required /></label>
        </template>
        <label v-if="kind === 'measures'">Measurement polarity<select v-model="form.polarity"><option value="higher_is_better">Higher is better</option><option value="lower_is_better">Lower is better</option><option value="target_range">Target range</option></select></label>
        <template v-if="kind === 'requires'">
          <label class="checkbox-label"><input v-model="form.hard" type="checkbox" /> Hard prerequisite</label>
          <label class="checkbox-label"><input v-model="form.thresholdEnabled" type="checkbox" /> Include satisfaction threshold</label>
          <label v-if="form.thresholdEnabled">Satisfaction threshold on [0, 1]<input v-model.number="form.threshold" type="number" min="0" max="1" step="0.05" required /></label>
        </template>
        <p v-if="validSources.length === 0" class="form-note">Add compatible endpoint node kinds for this relationship first.</p>
        <footer><button type="button" class="secondary-button" @click="emit('close')">Cancel</button><button type="submit" class="primary-button" :disabled="pending || !destination || (nativeCausal && !responseValid)">{{ pending ? 'Adding…' : 'Add relationship' }}</button></footer>
      </form>
    </div>
  </Teleport>
</template>

<style scoped>
.relationship-fields { margin-top: 14px; }
.relationship-dialog { width: min(680px, 100%); }
.native-relationship-dialog { width: min(1040px, 100%); }
.endpoint-fields { align-items: start; }
.node-picker { min-width: 0; margin: 0 !important; padding: 0 !important; }
.node-picker-options { display: grid; gap: 5px; }
.node-picker-options label { position: relative; display: block; margin: 0 !important; cursor: pointer; }
.node-picker-options input { position: absolute; opacity: 0; pointer-events: none; }
.node-picker-option { min-width: 0; min-height: 48px; display: grid; grid-template-columns: 26px minmax(0, 1fr); align-items: center; gap: 8px; padding: 8px 10px; border: 1px solid var(--line); border-radius: 5px; background: white; }
.node-picker-option > span:last-child { min-width: 0; display: grid; gap: 2px; }
.node-picker-option strong, .node-picker-option small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.node-picker-option strong { color: var(--ink); font-size: 12px; }
.node-picker-option small { color: var(--muted); font-size: 9px; text-transform: capitalize; }
.node-picker-options label:hover .node-picker-option { border-color: #aeb8b1; background: #f7f9f5; }
.node-picker-options input:checked + .node-picker-option { border-color: var(--green); background: var(--green-soft); box-shadow: inset 3px 0 var(--green); }
.node-picker-options input:focus-visible + .node-picker-option { outline: 2px solid #2a7059; outline-offset: 2px; }
.node-picker-options input:disabled + .node-picker-option { cursor: default; }
.node-picker-options input:disabled:not(:checked) + .node-picker-option { display: none; }
.native-response { display: grid; gap: 10px; }
.native-response > header { display: grid; gap: 3px; padding-bottom: 8px; border-bottom: 1px solid var(--line); }
.native-response > header strong { font-size: 14px; }
.native-response > header span { color: var(--muted); font-size: 12px; line-height: 1.5; }
.response-units { display: flex; flex-wrap: wrap; align-items: baseline; gap: 8px; margin: 0; color: var(--muted); font-size: 12px; }
.response-units > span:not([aria-hidden]) { padding: 2px 7px; border: 1px solid var(--line); border-radius: 4px; background: white; color: var(--ink); font-size: 11px; }
.response-units small { font-size: 11px; }</style>
