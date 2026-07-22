<script setup lang="ts">
import { ref, watch } from 'vue'
import type {
  Distribution,
  Estimate,
  EstimateSourceInput,
  EstimateSupport,
  SquiggleEstimateDefinition,
  Unit,
} from '../api/types'
import { defaultSquiggleDefinition } from '../domain/squiggleEstimate'
import SquiggleEstimateEditor from './SquiggleEstimateEditor.vue'

const props = defineProps<{
  modelValue: EstimateSourceInput
  existing: Estimate | null
  projectId: string | null
  support: EstimateSupport
  expectedUnit: Unit
}>()
const emit = defineEmits<{
  'update:modelValue': [source: EstimateSourceInput]
  validity: [valid: boolean]
}>()

const definition = ref(initialDefinition())

watch(() => [props.modelValue, props.existing, props.expectedUnit] as const, () => {
  const next = initialDefinition()
  if (next.source !== definition.value.source) definition.value = next
}, { deep: true })

function initialDefinition(): SquiggleEstimateDefinition {
  if (props.modelValue.type === 'squiggle') {
    return { ...props.modelValue.definition, target_unit: props.expectedUnit }
  }
  if (props.existing?.source?.type === 'squiggle') {
    return { ...props.existing.source.definition, target_unit: props.expectedUnit }
  }
  const distribution = props.existing?.distribution ?? (
    props.modelValue.type === 'distribution' ? props.modelValue.distribution : null
  )
  return distribution
    ? {
        source: distributionSource(distribution),
        seed: 42,
        sample_count: 2_048,
        target_unit: props.expectedUnit,
      }
    : defaultSquiggleDefinition(props.support, props.expectedUnit)
}

function updateDefinition(value: SquiggleEstimateDefinition) {
  definition.value = value
  emit('update:modelValue', { type: 'squiggle', definition: value })
}

function distributionSource(distribution: Distribution) {
  switch (distribution.type) {
    case 'point': return `pointMass(${distribution.value ?? 0})`
    case 'normal': return `normal(${distribution.mean ?? 0}, ${distribution.standard_deviation ?? 1})`
    case 'log_normal': return `lognormal(${distribution.location ?? 0}, ${distribution.scale ?? 0.25})`
    case 'beta': return `beta(${distribution.alpha ?? 2}, ${distribution.beta ?? 2})`
    case 'scaled_beta': {
      const lower = distribution.lower ?? 0
      const upper = distribution.upper ?? 1
      return `beta(${distribution.alpha ?? 2}, ${distribution.beta ?? 2}) * ${upper - lower} + ${lower}`
    }
    case 'empirical': return `SampleSet.fromList(${JSON.stringify(distribution.samples ?? [])})`
  }
  return 'normal(0, 1)'
}

</script>

<template>
  <div class="estimate-source-editor">
    <p v-if="existing && existing.source?.type !== 'squiggle'" class="legacy-source-note">This legacy {{ existing.source?.type ?? 'distribution' }} estimate has been translated to equivalent Squiggle source. Saving replaces the old authoring source.</p>
    <SquiggleEstimateEditor
      :model-value="definition"
      :project-id="projectId"
      :support="support"
      :expected-unit="expectedUnit"
      @update:model-value="updateDefinition"
      @validity="emit('validity', $event)"
    />
  </div>
</template>

<style scoped>
.estimate-source-editor { display: grid; gap: 12px; }
.legacy-source-note { margin: 0; padding: 8px 9px; border-left: 3px solid #bb7a2f; background: #fff8eb; color: #704516; font-size: 9px; line-height: 1.45; }
</style>