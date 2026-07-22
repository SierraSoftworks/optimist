<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, ref, watch } from 'vue'
import { Calculator, ChevronDown, Plus } from '@lucide/vue'
import { api } from '../api/client'
import type { FermiAssessment, FermiEstimateDefinition, Unit } from '../api/types'
import type { FermiComponentDraft, FermiSupport } from '../domain/fermiBuilder'
import { compileFermiEquation, FermiEquationError } from '../domain/fermiEquation'
import { evaluateSquigglePreview, SquigglePreviewError, type SquigglePreview } from '../domain/squigglePreview'
import { divideUnits, formatUnitExpression, parseUnitExpression, unitsEqual } from '../domain/unitExpression'
import FermiVariableEditor from './FermiVariableEditor.vue'

const props = defineProps<{
  projectId: string
  support: FermiSupport
  expectedUnit: Unit
  modelValue: FermiEstimateDefinition | null
  initialAssessment?: FermiAssessment | null
}>()
const emit = defineEmits<{
  'update:modelValue': [definition: FermiEstimateDefinition]
  dirty: []
}>()
let nextId = props.modelValue?.variables.length ?? 2
const open = ref(Boolean(props.modelValue))
const pending = ref(false)
const error = ref<string | null>(null)
const assessment = ref<FermiAssessment | null>(props.initialAssessment ?? null)
const squiggle = reactive<{
  status: 'idle' | 'pending' | 'ready' | 'error'
  result: SquigglePreview | null
  error: string | null
}>({ status: 'idle', result: null, error: null })
const equation = ref(props.modelValue?.equation ?? (
  props.support === 'probability' || props.support === 'signed' ? 'x * y' : 'x + y'
))
const goalUnit = ref(formatUnitExpression(props.expectedUnit))
const components = reactive<Array<FermiComponentDraft & { id: number }>>([
  ...(props.modelValue?.variables.map((variable, index) => ({
    id: index,
    name: variable.name,
    likely: variable.estimate,
    low: variable.uncertainty.type === 'three_point' ? variable.uncertainty.low : variable.estimate / 10,
    high: variable.uncertainty.type === 'three_point' ? variable.uncertainty.high : variable.estimate * 10,
    unit: variable.unit,
    mode: variable.uncertainty.type === 'three_point' ? 'pert' as const : 'order_of_magnitude' as const,
  })) ?? [initialVariable(0, 'x'), initialVariable(1, 'y')]),
])
const preview = computed(() => {
  try {
    const goal = parseUnitExpression(goalUnit.value)
    const compiled = compileFermiEquation(equation.value, components, props.support)
    return {
      compiled,
      goal,
      matchesGoal: unitsEqual(compiled.unit, goal),
      residual: divideUnits(compiled.unit, goal),
      error: null,
    }
  } catch (reason) {
    return {
      compiled: null,
      goal: null,
      matchesGoal: false,
      residual: null,
      error: reason instanceof Error ? reason.message : 'The equation is invalid.',
      issueVariables: reason instanceof FermiEquationError ? reason.variables : [],
    }
  }
})
const variableIssues = computed(() => {
  const names = components.map((component) => component.name.trim())
  return components.map((_, index) => {
    const name = names[index]!
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) return 'Use letters, digits, and underscores.'
    if (names.filter((candidate) => candidate === name).length > 1) return 'Variable name is duplicated.'
    if (preview.value.issueVariables?.includes(name)) return preview.value.error
    if (preview.value.compiled && !preview.value.compiled.referencedVariables.has(name)) return 'Variable is not used by the equation.'
    return null
  })
})
const recommendation = computed(() => {
  const value = assessment.value?.recommendation
  return value?.status === 'exact' || value?.status === 'moment_matched' ? value : null
})
const estimate = computed(() => assessment.value?.report.estimates[0] ?? null)
const standardDeviation = computed(() => estimate.value?.variance == null ? null : Math.sqrt(estimate.value.variance))
const invalidDraws = computed(() => {
  const invalid = assessment.value?.report.diagnostics.invalid_samples
  return invalid ? invalid.zero_denominator + invalid.non_finite_primitive + invalid.non_finite_result : 0
})
const goalMatchesSlot = computed(() => {
  try {
    return unitsEqual(parseUnitExpression(goalUnit.value), props.expectedUnit)
  } catch {
    return false
  }
})
const hasVariableIssues = computed(() => variableIssues.value.some(Boolean))
let squiggleRevision = 0
let squiggleTimer: ReturnType<typeof setTimeout> | undefined

watch(
  [open, equation, () => components.map((component) => ({ ...component }))],
  scheduleSquigglePreview,
  { deep: true, immediate: true },
)
onBeforeUnmount(() => clearTimeout(squiggleTimer))

function scheduleSquigglePreview() {
  const revision = ++squiggleRevision
  clearTimeout(squiggleTimer)
  squiggle.result = null
  squiggle.error = null
  if (!open.value || !preview.value.compiled) {
    squiggle.status = 'idle'
    return
  }
  squiggle.status = 'pending'
  squiggleTimer = setTimeout(async () => {
    try {
      const result = await evaluateSquigglePreview(
        equation.value,
        components,
        props.support,
        props.expectedUnit,
      )
      if (revision !== squiggleRevision) return
      squiggle.result = result
      squiggle.status = 'ready'
    } catch (reason) {
      if (revision !== squiggleRevision) return
      const location = reason instanceof SquigglePreviewError && reason.line !== null
        ? `Line ${reason.line}${reason.column === null ? '' : `, column ${reason.column}`}: `
        : ''
      squiggle.error = `${location}${reason instanceof Error ? reason.message : 'Squiggle could not evaluate this equation.'}`
      squiggle.status = 'error'
    }
  }, 180)
}

function intervalPosition(value: number) {
  const result = squiggle.result
  if (!result || result.p95 <= result.p05) return 50
  return Math.max(0, Math.min(100, 100 * (value - result.p05) / (result.p95 - result.p05)))
}

function supportWarning(result: SquigglePreview) {
  const probability = result.supportViolationProbability.toLocaleString(undefined, { style: 'percent', maximumFractionDigits: 1 })
  if (props.support === 'non_negative') return `${probability} of predicted values are negative. This model cannot be adopted for a non-negative quantity.`
  return `${probability} of predicted values fall outside the required support and are clamped in the preview.`
}

function initialVariable(id: number, name: string) {
  const likely = props.support === 'probability'
    ? (id === 0 ? 0.7 : 0.85)
    : props.support === 'signed'
      ? (id === 0 ? 0.4 : 0.8)
      : typeof props.support === 'object'
        ? (props.support.bounded.lower + props.support.bounded.upper) / 4
        : 1
  const formatted = formatUnitExpression(props.expectedUnit)
  return { id, name, likely, low: likely / 10, high: likely * 10, unit: formatted === '1' ? '' : formatted, mode: 'order_of_magnitude' as const }
}

function addVariable() {
  components.push(initialVariable(nextId, `v${nextId + 1}`))
  nextId += 1
  assessment.value = null
  emit('dirty')
}

function updateVariable(index: number, value: FermiComponentDraft & { id: number }) {
  components[index] = value
  assessment.value = null
  emit('dirty')
}

function removeVariable(index: number) {
  components.splice(index, 1)
  assessment.value = null
  emit('dirty')
}

watch(() => props.expectedUnit, (next, previous) => {
  try {
    if (unitsEqual(parseUnitExpression(goalUnit.value), previous)) goalUnit.value = formatUnitExpression(next)
  } catch {
    // Preserve a custom goal while the user resolves it.
  }
  assessment.value = null
}, { deep: true })

async function assess() {
  error.value = null
  assessment.value = null
  if (!preview.value.compiled || !preview.value.goal) {
    error.value = preview.value.error
    return
  }
  if (!preview.value.matchesGoal) {
    error.value = 'Resolve the derived and goal units before running Monte Carlo.'
    return
  }
  pending.value = true
  try {
    assessment.value = await api.assessFermi(props.projectId, {
      formula: preview.value.compiled.formula,
      support: props.support,
      expected_unit: preview.value.goal,
      monte_carlo: { seed: 42, minimum_samples: 2_000, maximum_samples: 20_000, absolute_tolerance: 0.001, relative_tolerance: 0.01 },
    })
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : 'The decomposition could not be assessed.'
  } finally {
    pending.value = false
  }
}

function useDefinition() {
  if (!recommendation.value || !assessment.value || !goalMatchesSlot.value || !preview.value.compiled) return
  emit('update:modelValue', {
    language: 'optimist_squiggle_v1',
    equation: equation.value.trim(),
    variables: components.map((component) => ({
      name: component.name.trim(),
      estimate: component.likely,
      unit: component.unit.trim(),
      uncertainty: component.mode === 'pert'
        ? { type: 'three_point' as const, low: component.low, high: component.high }
        : { type: 'order_of_magnitude' as const },
    })),
    formula: preview.value.compiled.formula,
    monte_carlo: assessment.value.report.diagnostics.criterion,
  })
}

function format(value: number | null | undefined) {
  return value == null ? 'Unavailable' : value.toLocaleString(undefined, { maximumSignificantDigits: 6 })
}
</script>

<template>
  <section class="fermi-assistant">
    <button type="button" class="fermi-toggle" :aria-expanded="open" @click="open = !open">
      <Calculator :size="16" />
      <span><strong>Fermi decomposition</strong><small>Define variables, units, and an equation</small></span>
      <ChevronDown :size="15" :class="{ rotated: open }" />
    </button>
    <div v-if="open" class="fermi-workspace">
      <div class="fermi-equation-fields">
        <label>Goal unit<input v-model="goalUnit" aria-label="Fermi goal unit" placeholder="pianos/day" @input="assessment = null; emit('dirty')" /></label>
        <label>Equation<input v-model="equation" class="code-input" aria-label="Fermi equation" placeholder="(x * y) + (z / a)" spellcheck="false" @input="assessment = null; emit('dirty')" /></label>
      </div>

      <div class="fermi-equation-status" :class="{ invalid: preview.error || !preview.matchesGoal }" aria-live="polite">
        <template v-if="preview.compiled">
          <div><span>Central estimate</span><strong>{{ format(preview.compiled.central) }}</strong></div>
          <div><span>Derived unit</span><strong>{{ formatUnitExpression(preview.compiled.unit) }}</strong></div>
          <div><span>Goal unit</span><strong>{{ preview.goal ? formatUnitExpression(preview.goal) : 'Invalid' }}</strong></div>
          <p v-if="!preview.matchesGoal">Unresolved dimension: {{ preview.residual ? formatUnitExpression(preview.residual) : 'unknown' }}. Adjust variable units or the equation.</p>
        </template>
        <p v-else>{{ preview.error }}</p>
      </div>

      <section v-if="squiggle.status !== 'idle'" class="squiggle-preview" aria-live="polite">
        <header><strong>Live predictive check</strong><span>Squiggle</span></header>
        <p v-if="squiggle.status === 'pending'" class="squiggle-state">Evaluating the current uncertainty model…</p>
        <p v-else-if="squiggle.status === 'error'" class="squiggle-state invalid">{{ squiggle.error }}</p>
        <template v-else-if="squiggle.result">
          <div class="squiggle-interval">
            <div><span>90% interval</span><strong>{{ format(squiggle.result.p05) }}–{{ format(squiggle.result.p95) }}</strong></div>
            <div class="squiggle-track" role="img" :aria-label="`90 percent of simulated values fall between ${format(squiggle.result.p05)} and ${format(squiggle.result.p95)}, with median ${format(squiggle.result.p50)}`">
              <span class="squiggle-middle" :style="{ left: `${intervalPosition(squiggle.result.p25)}%`, width: `${intervalPosition(squiggle.result.p75) - intervalPosition(squiggle.result.p25)}%` }" />
              <span class="squiggle-median" :style="{ left: `${intervalPosition(squiggle.result.p50)}%` }" />
            </div>
            <div class="squiggle-bounds"><span>{{ format(squiggle.result.p05) }}</span><span>{{ format(squiggle.result.p50) }}</span><span>{{ format(squiggle.result.p95) }}</span></div>
          </div>
          <dl>
            <div><dt>Median</dt><dd>{{ format(squiggle.result.p50) }}</dd></div>
            <div><dt>Expected value</dt><dd>{{ format(squiggle.result.mean) }}</dd></div>
            <div><dt>Standard deviation</dt><dd>{{ format(squiggle.result.standardDeviation) }}</dd></div>
          </dl>
          <p v-if="squiggle.result.supportViolationProbability > 0.001" class="squiggle-warning">{{ supportWarning(squiggle.result) }}</p>
          <small>{{ squiggle.result.samples.toLocaleString() }} deterministic samples · {{ squiggle.result.executionMilliseconds.toLocaleString() }} ms</small>
        </template>
      </section>

      <div class="fermi-components">
        <FermiVariableEditor
          v-for="(component, index) in components"
          :key="component.id"
          :model-value="component"
          :index="index"
          :issue="variableIssues[index]"
          :removable="components.length > 1"
          @update:model-value="updateVariable(index, $event)"
          @remove="removeVariable(index)"
        />
      </div>

      <div class="fermi-actions">
        <button type="button" class="secondary-button" @click="addVariable"><Plus :size="14" /> Add variable</button>
        <button type="button" class="secondary-button" :disabled="pending || !preview.compiled || !preview.matchesGoal || hasVariableIssues" @click="assess">{{ pending ? 'Assessing…' : 'Assess equation' }}</button>
      </div>

      <p v-if="error" class="form-error" role="alert">{{ error }}</p>
      <div v-if="assessment" class="fermi-result" aria-live="polite">
        <dl>
          <div><dt>Derived unit</dt><dd>{{ formatUnitExpression(assessment.compiled.unit) }}</dd></div>
          <div><dt>Expected value</dt><dd>{{ format(estimate?.mean) }}</dd></div>
          <div><dt>Standard deviation</dt><dd>{{ format(standardDeviation) }}</dd></div>
          <div v-if="recommendation"><dt>90% interval</dt><dd>{{ format(recommendation.interval.lower) }}–{{ format(recommendation.interval.upper) }}</dd></div>
          <div><dt>Simulation</dt><dd>{{ assessment.report.diagnostics.valid_samples.toLocaleString() }} samples · {{ assessment.report.diagnostics.status.replaceAll('_', ' ') }}</dd></div>
          <div><dt>Invalid draws</dt><dd>{{ invalidDraws }}</dd></div>
        </dl>
        <template v-if="recommendation">
          <p v-if="recommendation.status === 'moment_matched'">{{ recommendation.warning }}</p>
          <button v-if="goalMatchesSlot" type="button" class="primary-button" @click="useDefinition">Use Fermi equation</button>
          <p v-else>Standalone assessment only: the equation is valid for {{ formatUnitExpression(assessment.compiled.unit) }}, but this estimate slot expects {{ formatUnitExpression(expectedUnit) }}.</p>
        </template>
        <p v-else class="form-error">{{ assessment.recommendation.status === 'unavailable' ? assessment.recommendation.reason : '' }}</p>
      </div>
    </div>
  </section>
</template>

<style scoped>
.fermi-assistant { overflow: hidden; border: 1px solid #aeb9b1; border-radius: 6px; background: #f7f9f5; }
.fermi-toggle { width: 100%; min-height: 48px; display: grid; grid-template-columns: 20px minmax(0, 1fr) 16px; gap: 8px; align-items: center; padding: 8px 10px; border: 0; background: transparent; color: var(--green); text-align: left; }
.fermi-toggle:hover, .fermi-toggle[aria-expanded='true'] { background: var(--green-soft); }
.fermi-toggle > span { min-width: 0; display: grid; gap: 2px; }
.fermi-toggle strong { color: var(--ink); font-size: 10px; }
.fermi-toggle small { color: var(--muted); font-size: 9px; }
.fermi-toggle > svg:last-child { transition: transform .16s ease; }
.fermi-toggle > svg.rotated { transform: rotate(180deg); }
.fermi-workspace { display: grid; gap: 12px; padding: 12px; border-top: 1px solid var(--line); background: white; }
.fermi-equation-fields { display: grid; grid-template-columns: minmax(120px, .4fr) minmax(220px, 1fr); gap: 8px; }
.fermi-equation-status { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 1px; overflow: hidden; border: 1px solid #a8bfb2; border-radius: 5px; background: #a8bfb2; }
.fermi-equation-status > div { min-width: 0; display: grid; gap: 2px; padding: 8px; background: #f3f8f4; }
.fermi-equation-status span { color: var(--muted); font-size: 8px; }
.fermi-equation-status strong { overflow: hidden; color: var(--green); font: 10px 'IBM Plex Mono', monospace; text-overflow: ellipsis; white-space: nowrap; }
.fermi-equation-status > p { grid-column: 1 / -1; margin: 0; padding: 8px; background: #f3f8f4; color: var(--muted); font-size: 9px; line-height: 1.45; }
.fermi-equation-status.invalid { border-color: #d8a098; background: #d8a098; }
.fermi-equation-status.invalid > div, .fermi-equation-status.invalid > p { background: #fff8f6; }
.fermi-equation-status.invalid strong, .fermi-equation-status.invalid > p { color: #8c3429; }
.squiggle-preview { display: grid; gap: 10px; padding: 10px; border: 1px solid #a9b8c4; border-radius: 5px; background: #f5f8fa; }
.squiggle-preview header { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.squiggle-preview header strong { color: var(--ink); font-size: 10px; }
.squiggle-preview header span { padding: 2px 5px; border: 1px solid #91a5b4; border-radius: 3px; color: #36566c; font: 8px 'IBM Plex Mono', monospace; }
.squiggle-state { min-height: 28px; display: grid; align-items: center; margin: 0; color: var(--muted); font-size: 9px; }
.squiggle-state.invalid { color: #8c3429; }
.squiggle-interval { display: grid; gap: 5px; }
.squiggle-interval > div:first-child { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; }
.squiggle-interval span { color: var(--muted); font-size: 8px; }
.squiggle-interval strong { color: #24485f; font: 10px 'IBM Plex Mono', monospace; }
.squiggle-track { position: relative; height: 8px; border-radius: 2px; background: #dbe4e9; }
.squiggle-middle { position: absolute; top: 0; bottom: 0; background: #6e9bb7; }
.squiggle-median { position: absolute; top: -3px; bottom: -3px; width: 2px; background: #183d54; transform: translateX(-1px); }
.squiggle-bounds { display: flex; justify-content: space-between; gap: 8px; }
.squiggle-preview dl { grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; }
.squiggle-preview dl div { display: grid; gap: 2px; }
.squiggle-preview dd { overflow: hidden; color: #24485f; font: 9px 'IBM Plex Mono', monospace; text-overflow: ellipsis; white-space: nowrap; }
.squiggle-warning { margin: 0; padding: 7px 8px; border-left: 3px solid #bb7a2f; background: #fff8eb; color: #704516; font-size: 9px; line-height: 1.45; }
.squiggle-preview > small { color: var(--muted); font: 8px 'IBM Plex Mono', monospace; }
.fermi-components { display: grid; gap: 8px; }
.fermi-actions { display: flex; justify-content: space-between; gap: 8px; }
.fermi-result { display: grid; gap: 10px; padding: 10px; border: 1px solid #a8bfb2; border-radius: 5px; background: #f3f8f4; }
.fermi-result dl { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 7px 14px; }
.fermi-result dl div { display: grid; gap: 2px; }
.fermi-result p { margin: 0; color: var(--muted); font-size: 9px; line-height: 1.5; }
.fermi-result .primary-button { justify-self: end; }

@media (max-width: 760px) {
  .fermi-equation-fields { grid-template-columns: 1fr; }
  .fermi-equation-status { grid-template-columns: 1fr; }
  .fermi-equation-status > p { grid-column: 1; }
  .squiggle-preview dl { grid-template-columns: 1fr; }
  .fermi-actions { flex-wrap: wrap; }
}
</style>