<script setup lang="ts">
import { computed, reactive, watch } from 'vue'
import type { Distribution } from '../api/types'
import DistributionPreview from './DistributionPreview.vue'
import ParameterHelp from './ParameterHelp.vue'

type Family = Distribution['type']
type Support = 'probability' | 'non_negative' | 'signed' | 'real'

const props = withDefaults(defineProps<{
  modelValue: Distribution
  families: Family[]
  support: Support
  pointLabel?: string
}>(), { pointLabel: 'Value' })
const emit = defineEmits<{ 'update:modelValue': [distribution: Distribution] }>()
const form = reactive({
  family: props.modelValue.type,
  value: 0,
  mean: 0,
  standardDeviation: 1,
  location: 0,
  scale: 0.25,
  alpha: 2,
  beta: 2,
  lower: 0,
  upper: 1,
})

const pointMinimum = computed(() => props.support === 'signed' ? -1 : props.support === 'real' ? undefined : 0)
const pointMaximum = computed(() => props.support === 'signed' || props.support === 'probability' ? 1 : undefined)
const boundMinimum = computed(() => props.support === 'signed' ? -1 : props.support === 'real' ? undefined : 0)
const boundMaximum = computed(() => props.support === 'signed' || props.support === 'probability' ? 1 : undefined)
const previewDomain = computed<[number, number] | undefined>(() => {
  if (props.support === 'probability') return [0, 1]
  if (props.support === 'signed') return [-1, 1]
  if (props.support === 'non_negative' && form.family === 'point') {
    return [0, Math.max(1, form.value * 1.2)]
  }
  return undefined
})
const familyHelp = computed(() => {
  switch (form.family) {
    case 'point':
      return 'Use a Point estimate only when one exact value is appropriate. It adds no modeled uncertainty, so downstream results may look more certain than the evidence supports.'
    case 'normal':
      return 'Normal models symmetric additive variation around a mean. It allows negative and arbitrarily large values, so it is unsuitable for probabilities, durations, and costs.'
    case 'log_normal':
      return 'LogNormal models positive multiplicative variation. It is useful for lead times and costs where overruns can be much larger than underruns.'
    case 'beta':
      return 'Beta models an uncertain proportion between 0 and 1. It works well for success probabilities and normalized system states.'
    case 'scaled_beta':
      return 'Scaled Beta keeps every possible value inside explicit lower and upper bounds while allowing asymmetric uncertainty.'
  }
})

watch(
  () => props.modelValue,
  (distribution) => {
    form.family = distribution.type
    if (distribution.type === 'point') form.value = distribution.value ?? 0
    if (distribution.type === 'normal') {
      form.mean = distribution.mean ?? 0
      form.standardDeviation = distribution.standard_deviation ?? 1
    }
    if (distribution.type === 'log_normal') {
      form.location = distribution.location ?? 0
      form.scale = distribution.scale ?? 0.25
    }
    if (distribution.type === 'beta' || distribution.type === 'scaled_beta') {
      form.alpha = distribution.alpha ?? 2
      form.beta = distribution.beta ?? 2
    }
    if (distribution.type === 'scaled_beta') {
      form.lower = distribution.lower ?? 0
      form.upper = distribution.upper ?? 1
    }
  },
  { immediate: true, deep: true },
)

function selectedDistribution(): Distribution {
  switch (form.family) {
    case 'normal':
      return { type: 'normal', mean: form.mean, standard_deviation: form.standardDeviation }
    case 'log_normal':
      return { type: 'log_normal', location: form.location, scale: form.scale }
    case 'beta':
      return { type: 'beta', alpha: form.alpha, beta: form.beta }
    case 'scaled_beta':
      return {
        type: 'scaled_beta', alpha: form.alpha, beta: form.beta,
        lower: form.lower, upper: form.upper,
      }
    default:
      return { type: 'point', value: form.value }
  }
}

function changeFamily() {
  if (form.family === 'scaled_beta') {
    form.lower = props.support === 'signed' ? -1 : 0
    form.upper = 1
  }
  emitDistribution()
}

function emitDistribution() {
  emit('update:modelValue', selectedDistribution())
}

</script>

<template>
  <div class="distribution-editor">
    <label>
      <span class="field-label">Distribution <ParameterHelp label="Distribution family" :text="familyHelp" /></span>
      <select v-model="form.family" aria-label="Distribution" @change="changeFamily">
        <option v-if="families.includes('point')" value="point">Point</option>
        <option v-if="families.includes('normal')" value="normal">Normal</option>
        <option v-if="families.includes('log_normal')" value="log_normal">LogNormal</option>
        <option v-if="families.includes('beta')" value="beta">Beta</option>
        <option v-if="families.includes('scaled_beta')" value="scaled_beta">Scaled Beta</option>
      </select>
    </label>

    <label v-if="form.family === 'point'">
      <span class="field-label">{{ pointLabel }} <ParameterHelp label="Point value" text="The exact value used in every model run. Moving it shifts the marker; there is no spread or tail risk." /></span>
      <input v-model.number="form.value" type="number" :aria-label="pointLabel" :min="pointMinimum" :max="pointMaximum" step="any" required @input="emitDistribution" />
    </label>

    <div v-else-if="form.family === 'normal'" class="field-grid distribution-fields">
      <label>
        <span class="field-label">Mean <ParameterHelp label="Mean" text="The center and average. Raising it shifts the whole curve right without changing its width." /></span>
        <input v-model.number="form.mean" type="number" aria-label="Mean" step="any" required @input="emitDistribution" />
      </label>
      <label>
        <span class="field-label">Standard deviation <ParameterHelp label="Standard deviation" text="The additive spread in the same units as the estimate. About 68% falls within one standard deviation and 95% within two." /></span>
        <input v-model.number="form.standardDeviation" type="number" aria-label="Standard deviation" min="0.000001" step="any" required @input="emitDistribution" />
      </label>
    </div>

    <div v-else-if="form.family === 'log_normal'" class="field-grid distribution-fields">
      <label>
        <span class="field-label">Log location <ParameterHelp label="Log location" text="The natural logarithm of the median. Increasing it by 1 multiplies the median by about 2.72 rather than adding 1." /></span>
        <input v-model.number="form.location" type="number" aria-label="Log location" step="any" required @input="emitDistribution" />
      </label>
      <label>
        <span class="field-label">Log scale <ParameterHelp label="Log scale" text="Multiplicative uncertainty. Larger values widen the curve, increase upper-tail risk, and make expensive or slow overruns more plausible." /></span>
        <input v-model.number="form.scale" type="number" aria-label="Log scale" min="0.000001" step="any" required @input="emitDistribution" />
      </label>
    </div>

    <div v-else class="field-grid distribution-fields">
      <label>
        <span class="field-label">Alpha <ParameterHelp label="Alpha" text="Upward evidence or weight. Increasing alpha relative to beta shifts probability toward larger values; increasing both tightens the curve." /></span>
        <input v-model.number="form.alpha" type="number" aria-label="Alpha" min="0.000001" step="any" required @input="emitDistribution" />
      </label>
      <label>
        <span class="field-label">Beta <ParameterHelp label="Beta" text="Downward evidence or weight. Increasing beta relative to alpha shifts probability toward smaller values; increasing both tightens the curve." /></span>
        <input v-model.number="form.beta" type="number" aria-label="Beta" min="0.000001" step="any" required @input="emitDistribution" />
      </label>
      <template v-if="form.family === 'scaled_beta'">
        <label>
          <span class="field-label">Lower bound <ParameterHelp label="Lower bound" text="A hard minimum, not a confidence threshold. The model assigns zero probability below it." /></span>
          <input v-model.number="form.lower" type="number" aria-label="Lower bound" :min="boundMinimum" :max="boundMaximum" step="any" required @input="emitDistribution" />
        </label>
        <label>
          <span class="field-label">Upper bound <ParameterHelp label="Upper bound" text="A hard maximum, not a high percentile. The model assigns zero probability above it." /></span>
          <input v-model.number="form.upper" type="number" aria-label="Upper bound" :min="boundMinimum" :max="boundMaximum" step="any" required @input="emitDistribution" />
        </label>
      </template>
    </div>

    <DistributionPreview :distribution="modelValue" :domain="previewDomain" />
  </div>
</template>

<style scoped>
.distribution-editor { display: grid; gap: 14px; margin-top: 14px; }
.distribution-fields { margin: 0; }
</style>
