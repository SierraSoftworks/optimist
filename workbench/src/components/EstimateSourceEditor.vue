<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type {
  Distribution,
  Estimate,
  EstimateSourceInput,
  FermiEstimateDefinition,
  Unit,
} from '../api/types'
import type { FermiSupport } from '../domain/fermiBuilder'
import DistributionEditor from './DistributionEditor.vue'
import DistributionPreview from './DistributionPreview.vue'
import FermiEstimateAssistant from './FermiEstimateAssistant.vue'

type Family = Distribution['type']

const props = withDefaults(defineProps<{
  modelValue: EstimateSourceInput
  existing: Estimate | null
  projectId: string | null
  families: Family[]
  support: FermiSupport
  expectedUnit: Unit
  pointLabel?: string
  allowFermi?: boolean
  minimum?: number
  maximum?: number
}>(), { pointLabel: 'Value', allowFermi: true })
const emit = defineEmits<{
  'update:modelValue': [source: EstimateSourceInput]
  validity: [valid: boolean]
}>()
const mode = ref<'distribution' | 'fermi'>(
  props.existing?.source?.type === 'fermi' || props.modelValue.type === 'fermi'
    ? 'fermi'
    : 'distribution',
)
const distribution = ref<Distribution>(
  props.modelValue.type === 'distribution'
    ? props.modelValue.distribution
    : props.existing?.distribution ?? { type: 'point', value: 0 },
)
const definition = ref<FermiEstimateDefinition | null>(
  props.modelValue.type === 'fermi'
    ? props.modelValue.definition
    : props.existing?.source?.type === 'fermi'
      ? props.existing.source.definition
      : null,
)
const existingAssessment = computed(() =>
  props.existing?.source?.type === 'fermi' ? props.existing.source.assessment : null,
)

watch(() => props.modelValue, (source) => {
  mode.value = source.type
  if (source.type === 'distribution') distribution.value = source.distribution
  else definition.value = source.definition
}, { deep: true })

function selectMode(next: 'distribution' | 'fermi') {
  mode.value = next
  if (next === 'distribution') {
    emit('update:modelValue', { type: 'distribution', distribution: distribution.value })
    emit('validity', true)
  } else if (definition.value) {
    emit('update:modelValue', { type: 'fermi', definition: definition.value })
    emit('validity', true)
  } else {
    emit('validity', false)
  }
}

function updateDistribution(value: Distribution) {
  distribution.value = value
  emit('update:modelValue', { type: 'distribution', distribution: value })
  emit('validity', true)
}

function updateDefinition(value: FermiEstimateDefinition) {
  definition.value = value
  emit('update:modelValue', { type: 'fermi', definition: value })
  emit('validity', true)
}

function invalidateDefinition() {
  if (mode.value === 'fermi') {
    definition.value = null
    emit('validity', false)
  }
}
</script>

<template>
  <div class="estimate-source-editor">
    <div v-if="allowFermi" class="source-mode" role="group" aria-label="Estimate source">
      <button type="button" :aria-pressed="mode === 'distribution'" @click="selectMode('distribution')">Distribution</button>
      <button type="button" :aria-pressed="mode === 'fermi'" @click="selectMode('fermi')">Fermi equation</button>
    </div>
    <DistributionEditor
      v-if="mode === 'distribution'"
      :model-value="distribution"
      :families="families"
      :support="support"
      :point-label="pointLabel"
      :minimum="minimum"
      :maximum="maximum"
      @update:model-value="updateDistribution"
    />
    <div v-else class="fermi-source">
      <div v-if="existing?.source?.type === 'fermi' && definition" class="stored-fermi-result">
        <div><strong>Stored effective result</strong><span>Estimate revision {{ existing.revision }}</span></div>
        <DistributionPreview :distribution="existing.distribution" />
      </div>
      <FermiEstimateAssistant
        v-if="projectId"
        :project-id="projectId"
        :support="support"
        :expected-unit="expectedUnit"
        :model-value="definition"
        :initial-assessment="existingAssessment"
        @dirty="invalidateDefinition"
        @update:model-value="updateDefinition"
      />
      <p v-if="!definition" class="form-note">Assess and accept the equation before saving this estimate.</p>
    </div>
  </div>
</template>

<style scoped>
.estimate-source-editor { display: grid; gap: 12px; }
.source-mode { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); padding: 3px; border: 1px solid var(--line); border-radius: 6px; background: #edf0eb; }
.source-mode button { min-height: 30px; border: 0; border-radius: 4px; background: transparent; color: var(--muted); font-size: 10px; font-weight: 700; }
.source-mode button[aria-pressed='true'] { background: white; color: var(--ink); box-shadow: 0 1px 4px rgba(30, 40, 34, .12); }
.fermi-source { display: grid; gap: 10px; }
.stored-fermi-result { padding: 10px; border: 1px solid #a8bfb2; border-radius: 6px; background: #f3f8f4; }
.stored-fermi-result > div { display: flex; justify-content: space-between; gap: 8px; }
.stored-fermi-result strong { font-size: 10px; }
.stored-fermi-result span { color: var(--muted); font-size: 8px; }
.stored-fermi-result :deep(.distribution-preview) { margin-top: 8px; }
</style>