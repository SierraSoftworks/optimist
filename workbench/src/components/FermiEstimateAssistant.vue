<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { Calculator, ChevronDown, Plus } from '@lucide/vue'
import { api } from '../api/client'
import type { Distribution, FermiAssessment, Unit } from '../api/types'
import type { FermiComponentDraft, FermiSupport } from '../domain/fermiBuilder'
import { compileFermiEquation, FermiEquationError, fermiEquationProvenance } from '../domain/fermiEquation'
import { divideUnits, formatUnitExpression, parseUnitExpression, unitsEqual } from '../domain/unitExpression'
import FermiVariableEditor from './FermiVariableEditor.vue'

const props = defineProps<{ projectId: string; support: FermiSupport; expectedUnit: Unit }>()
const emit = defineEmits<{ apply: [distribution: Distribution, provenance: string] }>()
let nextId = 2
const open = ref(false)
const pending = ref(false)
const error = ref<string | null>(null)
const assessment = ref<FermiAssessment | null>(null)
const equation = ref(props.support === 'non_negative' ? 'x + y' : 'x * y')
const goalUnit = ref(formatUnitExpression(props.expectedUnit))
const components = reactive<Array<FermiComponentDraft & { id: number }>>([
  initialVariable(0, 'x'),
  initialVariable(1, 'y'),
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

function initialVariable(id: number, name: string) {
  const likely = props.support === 'probability' ? (id === 0 ? 0.7 : 0.85) : props.support === 'signed' ? (id === 0 ? 0.4 : 0.8) : 1
  const formatted = formatUnitExpression(props.expectedUnit)
  return { id, name, likely, low: likely / 10, high: likely * 10, unit: formatted === '1' ? '' : formatted, mode: 'order_of_magnitude' as const }
}

function addVariable() {
  components.push(initialVariable(nextId, `v${nextId + 1}`))
  nextId += 1
  assessment.value = null
}

function updateVariable(index: number, value: FermiComponentDraft & { id: number }) {
  components[index] = value
  assessment.value = null
}

function removeVariable(index: number) {
  components.splice(index, 1)
  assessment.value = null
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

function apply() {
  if (!recommendation.value || !assessment.value || !goalMatchesSlot.value) return
  emit('apply', recommendation.value.distribution, fermiEquationProvenance(equation.value, components, assessment.value.report.diagnostics.valid_samples))
  open.value = false
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
        <label>Goal unit<input v-model="goalUnit" aria-label="Fermi goal unit" placeholder="pianos/day" @input="assessment = null" /></label>
        <label>Equation<input v-model="equation" class="code-input" aria-label="Fermi equation" placeholder="(x * y) + (z / a)" spellcheck="false" @input="assessment = null" /></label>
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
          <button v-if="goalMatchesSlot" type="button" class="primary-button" @click="apply">Use suggested distribution</button>
          <p v-else>Standalone assessment only: the equation is valid for {{ formatUnitExpression(assessment.compiled.unit) }}, but this estimate slot expects {{ formatUnitExpression(expectedUnit) }}.</p>
        </template>
        <p v-else class="form-error">{{ assessment.recommendation.status === 'unavailable' ? assessment.recommendation.reason : '' }}</p>
      </div>
    </div>
  </section>
</template>