<script setup lang="ts">
import { computed, ref } from 'vue'
import { ChevronDown, Trash2 } from '@lucide/vue'
import type { FermiComponentDraft } from '../domain/fermiBuilder'
import { formatHumanNumber, parseHumanNumber } from '../domain/humanNumber'

const props = defineProps<{
  modelValue: FermiComponentDraft & { id: number }
  index: number
  issue?: string | null
  removable: boolean
}>()
const emit = defineEmits<{
  'update:modelValue': [value: FermiComponentDraft & { id: number }]
  remove: []
}>()
const detailed = ref(false)
const valueError = ref<string | null>(null)
const quickRange = computed(() =>
  `${formatHumanNumber(props.modelValue.likely / 10)} to ${formatHumanNumber(props.modelValue.likely * 10)}`,
)

function update(patch: Partial<FermiComponentDraft>) {
  emit('update:modelValue', { ...props.modelValue, ...patch })
}

function updateLikely(source: string) {
  try {
    const likely = parseHumanNumber(source)
    update((props.modelValue.mode ?? 'order_of_magnitude') === 'order_of_magnitude'
      ? { likely, low: likely / 10, high: likely * 10 }
      : { likely })
    valueError.value = null
  } catch (reason) {
    valueError.value = reason instanceof Error ? reason.message : 'Invalid estimate.'
  }
}
</script>

<template>
  <fieldset class="fermi-variable" :class="{ invalid: issue || valueError }">
    <legend>Variable {{ index + 1 }}</legend>
    <div class="fermi-variable-quick">
      <label>
        Variable
        <input :value="modelValue.name" :aria-label="`Variable ${index + 1} name`" placeholder="people_per_household" @input="update({ name: ($event.target as HTMLInputElement).value })" />
      </label>
      <label>
        Estimate
        <input :value="formatHumanNumber(modelValue.likely)" inputmode="decimal" :aria-label="`Variable ${index + 1} estimate`" placeholder="1.5M" @change="updateLikely(($event.target as HTMLInputElement).value)" />
      </label>
      <label>
        Unit
        <input :value="modelValue.unit" :aria-label="`Variable ${index + 1} unit`" placeholder="people/household" @input="update({ unit: ($event.target as HTMLInputElement).value })" />
      </label>
      <button type="button" class="icon-button fermi-detail-toggle" :aria-label="`Edit uncertainty for variable ${index + 1}`" :aria-expanded="detailed" @click="detailed = !detailed"><ChevronDown :size="14" :class="{ rotated: detailed }" /></button>
      <button v-if="removable" type="button" class="icon-button fermi-remove" :aria-label="`Remove variable ${index + 1}`" @click="emit('remove')"><Trash2 :size="14" /></button>
    </div>
    <p v-if="valueError || issue" class="fermi-variable-issue" role="alert">{{ valueError ?? issue }}</p>
    <p v-else-if="(modelValue.mode ?? 'order_of_magnitude') === 'order_of_magnitude'" class="fermi-variable-range">90% interval defaults to {{ quickRange }} {{ modelValue.unit || 'dimensionless' }}.</p>
    <div v-if="detailed" class="fermi-variable-detail">
      <label>
        Uncertainty
        <select :value="modelValue.mode ?? 'order_of_magnitude'" :aria-label="`Variable ${index + 1} uncertainty`" @change="update({ mode: ($event.target as HTMLSelectElement).value as FermiComponentDraft['mode'] })">
          <option value="order_of_magnitude">±1 order of magnitude</option>
          <option value="pert">Custom three-point range</option>
        </select>
      </label>
      <div v-if="modelValue.mode === 'pert'" class="fermi-range">
        <label>Low<input :value="modelValue.low" type="number" step="any" :aria-label="`Variable ${index + 1} low`" @input="update({ low: Number(($event.target as HTMLInputElement).value) })" /></label>
        <label>Likely<input :value="modelValue.likely" type="number" step="any" :aria-label="`Variable ${index + 1} likely`" @input="update({ likely: Number(($event.target as HTMLInputElement).value) })" /></label>
        <label>High<input :value="modelValue.high" type="number" step="any" :aria-label="`Variable ${index + 1} high`" @input="update({ high: Number(($event.target as HTMLInputElement).value) })" /></label>
      </div>
    </div>
  </fieldset>
</template>