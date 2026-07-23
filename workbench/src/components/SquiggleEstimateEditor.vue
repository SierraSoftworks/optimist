<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, ref, watch } from 'vue'
import { Braces, CheckCircle2, LoaderCircle } from '@lucide/vue'
import { api } from '../api/client'
import type {
  EstimateSupport,
  SquiggleAssessmentResult,
  SquiggleEstimateDefinition,
  Unit,
} from '../api/types'
import { formatUnitExpression } from '../domain/unitExpression'
import SquiggleEditorIsland from './SquiggleEditorIsland.vue'

const props = defineProps<{
  projectId: string | null
  modelValue: SquiggleEstimateDefinition
  support: EstimateSupport
  expectedUnit: Unit
}>()
const emit = defineEmits<{
  'update:modelValue': [definition: SquiggleEstimateDefinition]
  validity: [valid: boolean]
  assessment: [result: SquiggleAssessmentResult | null]
}>()

const source = ref(props.modelValue.source)
const preview = reactive<{
  status: 'idle' | 'pending' | 'ready' | 'error'
  result: SquiggleAssessmentResult | null
  error: string | null
}>({ status: 'idle', result: null, error: null })
let timer: ReturnType<typeof setTimeout> | undefined
let revision = 0

const definition = computed<SquiggleEstimateDefinition>(() => ({
  source: source.value,
  seed: props.modelValue.seed,
  sample_count: props.modelValue.sample_count,
  target_unit: props.expectedUnit,
}))

watch(() => props.modelValue, (value) => {
  if (value.source !== source.value) source.value = value.source
}, { deep: true })
watch([source, () => props.projectId, () => props.expectedUnit], schedule, {
  deep: true,
  immediate: true,
})
onBeforeUnmount(() => clearTimeout(timer))

function schedule() {
  const current = ++revision
  clearTimeout(timer)
  preview.result = null
  emit('assessment', null)
  preview.error = null
  if (!props.projectId || !source.value.trim()) {
    preview.status = 'idle'
    emit('validity', false)
    return
  }
  preview.status = 'pending'
  emit('validity', false)
  timer = setTimeout(async () => {
    try {
      const result = await api.assessSquiggle(props.projectId!, definition.value, props.support)
      if (current !== revision) return
      preview.result = result
      emit('assessment', result)
      preview.status = 'ready'
      emit('update:modelValue', definition.value)
      emit('validity', result.predictive_checks.support_violation_draws === 0)
    } catch (reason) {
      if (current !== revision) return
      preview.error = reason instanceof Error ? reason.message : 'Squiggle evaluation failed.'
      preview.status = 'error'
      emit('validity', false)
    }
  }, 250)
}

</script>

<template>
  <section class="squiggle-estimate-editor">
    <header>
      <div><Braces :size="18" /><span><strong>Squiggle estimate</strong></span></div>
      <code>{{ formatUnitExpression(expectedUnit) }}</code>
    </header>
    <SquiggleEditorIsland v-model="source" label="Squiggle source" :sample-count="definition.sample_count" :seed="definition.seed" />
    <div class="evaluation-state" :data-status="preview.status" aria-live="polite">
      <template v-if="preview.status === 'pending'"><LoaderCircle class="spin" :size="15" /><span>Evaluating on the backend…</span></template>
      <template v-else-if="preview.status === 'error'"><span>{{ preview.error }}</span></template>
      <template v-else-if="preview.result">
        <CheckCircle2 :size="15" />
        <span>Validated · {{ preview.result.assessment.sample_count.toLocaleString() }} effective samples</span>
      </template>
      <span v-else>Enter a calculation returning a number or distribution.</span>
    </div>
    <section v-if="preview.result && (preview.result.predictive_checks.invalid_draws || preview.result.predictive_checks.support_violation_draws)" class="predictive-checks" data-valid="false">
      <header><strong>Validation issue</strong><span>{{ preview.result.predictive_checks.valid_draws.toLocaleString() }} / {{ preview.result.predictive_checks.attempted_draws.toLocaleString() }} valid draws</span></header>
      <dl>
        <div><dt>Invalid draws</dt><dd>{{ preview.result.predictive_checks.invalid_draws.toLocaleString() }}</dd></div>
        <div><dt>Outside support</dt><dd>{{ (preview.result.predictive_checks.support_violation_probability * 100).toFixed(2) }}%</dd></div>
      </dl>
      <p v-if="preview.result.predictive_checks.support_violation_draws">{{ preview.result.predictive_checks.support_violation_draws.toLocaleString() }} retained draws fall outside this estimate slot. Revise or bound the calculation before saving.</p>
    </section>
  </section>
</template>

<style scoped>
.squiggle-estimate-editor { display: grid; gap: 12px; }
.squiggle-estimate-editor > header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.squiggle-estimate-editor > header > div { display: flex; align-items: center; gap: 8px; }
.squiggle-estimate-editor > header span { display: grid; gap: 2px; }
.squiggle-estimate-editor > header strong { font-size: 14px; }
.squiggle-estimate-editor > header code { color: var(--green); font-size: 12px; }
.evaluation-state { min-height: 40px; display: flex; align-items: center; gap: 8px; padding: 9px 11px; border: 1px solid var(--line); border-radius: 5px; color: var(--muted); font-size: 12px; }
.evaluation-state[data-status='ready'] { border-color: #a8bfb2; background: #f3f8f4; color: var(--green); }
.evaluation-state[data-status='error'] { border-color: #d8a098; background: #fff8f6; color: #8c3429; }
.predictive-checks { display: grid; gap: 10px; padding: 12px; border: 1px solid #d8a098; border-radius: 5px; background: #fff8f6; }
.predictive-checks[data-valid='false'] { border-color: #d8a098; background: #fff8f6; }
.predictive-checks > header { display: flex; justify-content: space-between; gap: 8px; font-size: 12px; }
.predictive-checks > header span { color: var(--muted); }
.predictive-checks dl { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
.predictive-checks dl div { display: grid; gap: 2px; }
.predictive-checks p { margin: 0; color: #8c3429; font-size: 12px; line-height: 1.45; }

@media (max-width: 760px) {
  .predictive-checks > header { align-items: flex-start; flex-direction: column; }
}
</style>
