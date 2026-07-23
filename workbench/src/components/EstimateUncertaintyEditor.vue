<script setup lang="ts">
import { reactive, watch } from 'vue'
import type { EstimateUncertainty } from '../api/types'

const model = defineModel<EstimateUncertainty>({ required: true })
const draft = reactive({ epistemic: '', process: '', measurement: '' })

watch(
  model,
  (value) => Object.assign(draft, {
    epistemic: value.epistemic ?? '',
    process: value.process ?? '',
    measurement: value.measurement ?? '',
  }),
  { immediate: true },
)

function update(field: keyof EstimateUncertainty, event: Event) {
  draft[field] = (event.target as HTMLTextAreaElement).value
  model.value = {
    ...draft,
  }
}
</script>

<template>
  <fieldset class="uncertainty-editor">
    <legend>Uncertainty sources</legend>
    <label>
      Epistemic
      <textarea :value="draft.epistemic" rows="2" placeholder="Knowledge gaps and model assumptions" @input="update('epistemic', $event)"></textarea>
    </label>
    <label>
      Process
      <textarea :value="draft.process" rows="2" placeholder="Variation between future outcomes" @input="update('process', $event)"></textarea>
    </label>
    <label>
      Measurement
      <textarea :value="draft.measurement" rows="2" placeholder="Observation and resolution error" @input="update('measurement', $event)"></textarea>
    </label>
  </fieldset>
</template>

<style scoped>
.uncertainty-editor {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
  margin: 0;
  padding: 10px 0 0;
  border: 0;
  border-top: 1px solid var(--line);
}

.uncertainty-editor legend {
  padding: 0 8px 0 0;
  color: var(--muted);
  font-size: 9px;
  font-weight: 700;
  text-transform: uppercase;
}

@media (max-width: 760px) {
  .uncertainty-editor { grid-template-columns: 1fr; }
}
</style>