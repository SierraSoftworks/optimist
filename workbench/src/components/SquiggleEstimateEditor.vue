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
      const result = await api.assessSquiggle(props.projectId!, definition.value)
      if (current !== revision) return
      if (!fitsSupport(result, props.support)) {
        throw new Error('The evaluated result has samples outside this estimate slot support.')
      }
      preview.result = result
      emit('assessment', result)
      preview.status = 'ready'
      emit('update:modelValue', definition.value)
      emit('validity', true)
    } catch (reason) {
      if (current !== revision) return
      preview.error = reason instanceof Error ? reason.message : 'Squiggle evaluation failed.'
      preview.status = 'error'
      emit('validity', false)
    }
  }, 250)
}

function fitsSupport(result: SquiggleAssessmentResult, support: EstimateSupport) {
  const distribution = result.effective_distribution
  const samples = distribution.type === 'point'
    ? [distribution.value ?? Number.NaN]
    : distribution.samples ?? []
  if (!samples.length || samples.some((value) => !Number.isFinite(value))) return false
  if (support === 'real') return true
  if (support === 'non_negative') return samples.every((value) => value >= 0)
  const bounds = support === 'probability'
    ? [0, 1]
    : support === 'signed'
      ? [-1, 1]
      : [support.bounded.lower, support.bounded.upper]
  return samples.every((value) => value >= bounds[0]! && value <= bounds[1]!)
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

@media (max-width: 760px) {
  .squiggle-summary { grid-template-columns: 1fr; }
}
</style>
