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
const standardDeviation = computed(() => {
  const variance = preview.result?.assessment.variance
  return variance == null ? null : Math.sqrt(variance)
})

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

function format(value: number | null | undefined) {
  return value == null ? 'Undefined' : value.toLocaleString(undefined, { maximumSignificantDigits: 6 })
}
</script>

<template>
  <section class="squiggle-estimate-editor">
    <header>
      <div><Braces :size="17" /><span><strong>Squiggle estimate</strong><small>Evaluated by Optimist's Rust runtime</small></span></div>
      <code>{{ formatUnitExpression(expectedUnit) }}</code>
    </header>
    <label><span>Calculation</span></label>
    <SquiggleEditorIsland v-model="source" label="Squiggle source" :sample-count="definition.sample_count" :seed="definition.seed" />
    <div class="evaluation-state" :data-status="preview.status" aria-live="polite">
      <template v-if="preview.status === 'pending'"><LoaderCircle class="spin" :size="15" /><span>Evaluating on the backend…</span></template>
      <template v-else-if="preview.status === 'error'"><span>{{ preview.error }}</span></template>
      <template v-else-if="preview.result">
        <CheckCircle2 :size="15" />
        <span>{{ preview.result.assessment.family }} · {{ preview.result.assessment.sample_count.toLocaleString() }} effective samples</span>
      </template>
      <span v-else>Enter a calculation returning a number or distribution.</span>
    </div>
    <dl v-if="preview.result" class="squiggle-summary">
      <div><dt>Expected value</dt><dd>{{ format(preview.result.assessment.mean) }}</dd></div>
      <div><dt>Standard deviation</dt><dd>{{ format(standardDeviation) }}</dd></div>
      <div><dt>90% interval</dt><dd>{{ format(preview.result.assessment.p05) }}–{{ format(preview.result.assessment.p95) }}</dd></div>
      <div><dt>Median</dt><dd>{{ format(preview.result.assessment.p50) }}</dd></div>
    </dl>
    <section v-if="preview.result" class="predictive-checks" :data-valid="preview.result.predictive_checks.support_violation_draws === 0">
      <header><strong>Prior-predictive checks</strong><span>{{ preview.result.predictive_checks.valid_draws.toLocaleString() }} / {{ preview.result.predictive_checks.attempted_draws.toLocaleString() }} valid draws</span></header>
      <dl>
        <div><dt>Invalid draws</dt><dd>{{ preview.result.predictive_checks.invalid_draws.toLocaleString() }}</dd></div>
        <div><dt>Outside support</dt><dd>{{ (preview.result.predictive_checks.support_violation_probability * 100).toFixed(2) }}%</dd></div>
      </dl>
      <div class="representative-outcomes">
        <span v-for="outcome in preview.result.predictive_checks.representative_outcomes" :key="outcome.percentile"><small>P{{ outcome.percentile * 100 }}</small><strong>{{ format(outcome.value) }}</strong></span>
      </div>
      <p v-if="preview.result.predictive_checks.support_violation_draws">{{ preview.result.predictive_checks.support_violation_draws.toLocaleString() }} retained draws fall outside this estimate slot. Revise or bound the calculation before saving.</p>
    </section>
  </section>
</template>

<style scoped>
.squiggle-estimate-editor { display: grid; gap: 12px; }
.squiggle-estimate-editor > header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.squiggle-estimate-editor > header > div { display: flex; align-items: center; gap: 8px; }
.squiggle-estimate-editor > header span { display: grid; gap: 2px; }
.squiggle-estimate-editor > header strong { font-size: 11px; }
.squiggle-estimate-editor > header small { color: var(--muted); font-size: 8px; }
.squiggle-estimate-editor > header code { color: var(--green); font-size: 9px; }
.evaluation-state { min-height: 34px; display: flex; align-items: center; gap: 7px; padding: 8px 9px; border: 1px solid var(--line); border-radius: 5px; color: var(--muted); font-size: 9px; }
.evaluation-state[data-status='ready'] { border-color: #a8bfb2; background: #f3f8f4; color: var(--green); }
.evaluation-state[data-status='error'] { border-color: #d8a098; background: #fff8f6; color: #8c3429; }
.squiggle-summary { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px 14px; padding: 10px; border: 1px solid #a8bfb2; border-radius: 5px; background: #f3f8f4; }
.squiggle-summary div { display: grid; gap: 2px; }
.squiggle-summary dd { color: var(--green); font: 9px 'IBM Plex Mono', monospace; }
.predictive-checks { display: grid; gap: 8px; padding: 10px; border: 1px solid #a8bfb2; border-radius: 5px; background: #f7faf7; }
.predictive-checks[data-valid='false'] { border-color: #d8a098; background: #fff8f6; }
.predictive-checks > header { display: flex; justify-content: space-between; gap: 8px; font-size: 9px; }
.predictive-checks > header span { color: var(--muted); }
.predictive-checks dl { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
.predictive-checks dl div { display: grid; gap: 2px; }
.representative-outcomes { display: grid; grid-template-columns: repeat(3, 1fr); gap: 6px; }
.representative-outcomes > span { display: grid; gap: 2px; padding: 6px; border: 1px solid var(--line); border-radius: 4px; background: white; }
.representative-outcomes small { color: var(--muted); font-size: 7px; }
.representative-outcomes strong { font: 9px 'IBM Plex Mono', monospace; }
.predictive-checks p { margin: 0; color: #8c3429; font-size: 9px; line-height: 1.45; }

@media (max-width: 760px) {
  .squiggle-summary { grid-template-columns: 1fr; }
}
</style>
