<script setup lang="ts">
import { ref, watch } from 'vue'
import type {
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
  if (props.existing) {
    return { ...props.existing.source.definition, target_unit: props.expectedUnit }
  }
  return props.modelValue.definition
    ? { ...props.modelValue.definition, target_unit: props.expectedUnit }
    : defaultSquiggleDefinition(props.support, props.expectedUnit)
}

function updateDefinition(value: SquiggleEstimateDefinition) {
  definition.value = value
  emit('update:modelValue', { type: 'squiggle', definition: value })
}

</script>

<template>
  <div class="estimate-source-editor">
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
</style>