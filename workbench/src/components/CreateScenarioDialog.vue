<script setup lang="ts">
import { computed, reactive, watch } from 'vue'
import { X } from '@lucide/vue'
import type { GraphNode, Scenario, ScenarioDraft } from '../api/types'

const props = defineProps<{
  open: boolean
  pending: boolean
  nodes: GraphNode[]
  scenario: Scenario | null
}>()
const emit = defineEmits<{ close: []; submit: [scenario: ScenarioDraft] }>()
const form = reactive({
  title: '',
  name: '',
  rationale: '',
  planningHorizon: 12,
  objectives: {} as Record<string, boolean>,
  importance: {} as Record<string, number>,
  candidates: {} as Record<string, boolean>,
  seed: 42,
  minimumSamples: 100,
  maximumSamples: 1000,
  absoluteTolerance: 0.01,
  relativeTolerance: 0.01,
})
const outcomes = computed(() => props.nodes.filter((node) => node.payload.kind === 'outcome'))
const interventions = computed(() => props.nodes.filter((node) => node.payload.kind === 'intervention'))
const generatedName = computed(() => form.title
  .trim()
  .toLocaleLowerCase()
  .replace(/[^a-z0-9]+/g, '_')
  .replace(/^_|_$/g, ''),
)
const selectedObjectives = computed(() => outcomes.value.filter((node) => form.objectives[node.id]))
const selectedCandidates = computed(() => interventions.value.filter((node) => form.candidates[node.id]))

watch(
  () => [props.open, props.scenario] as const,
  ([open, scenario]) => {
    if (!open) return
    Object.assign(form, {
      title: scenario?.title ?? '', name: scenario?.name ?? '',
      rationale: scenario?.rationale ?? '', planningHorizon: scenario?.planning_horizon ?? 12,
      objectives: {}, importance: {}, candidates: {},
      seed: scenario?.monte_carlo.seed ?? 42,
      minimumSamples: scenario?.monte_carlo.minimum_samples ?? 100,
      maximumSamples: scenario?.monte_carlo.maximum_samples ?? 1000,
      absoluteTolerance: scenario?.monte_carlo.absolute_tolerance ?? 0.01,
      relativeTolerance: scenario?.monte_carlo.relative_tolerance ?? 0.01,
    })
    for (const outcome of outcomes.value) form.importance[outcome.id] = 1
    for (const objective of scenario?.objectives ?? []) {
      form.objectives[objective.outcome_id] = true
      form.importance[objective.outcome_id] = objective.importance
    }
    for (const candidate of scenario?.candidate_interventions ?? []) {
      form.candidates[candidate] = true
    }
  },
)

function submit() {
  const title = form.title.trim()
  const name = form.name.trim() || generatedName.value
  if (!title || !name || !selectedObjectives.value.length || !selectedCandidates.value.length) return
  emit('submit', {
    name,
    title,
    rationale: form.rationale,
    objectives: selectedObjectives.value.map((node) => ({
      outcome_id: node.id,
      direction: node.payload.kind === 'outcome' && node.payload.properties.direction === 'minimize'
        ? 'minimize'
        : 'maximize',
      importance: form.importance[node.id] ?? 1,
    })),
    planning_horizon: form.planningHorizon,
    budgets: props.scenario?.budgets ?? [],
    candidate_interventions: selectedCandidates.value.map((node) => node.id),
    monte_carlo: {
      seed: form.seed,
      minimum_samples: form.minimumSamples,
      maximum_samples: form.maximumSamples,
      absolute_tolerance: form.absoluteTolerance,
      relative_tolerance: form.relativeTolerance,
    },
    ...(props.scenario?.scalar_preferences
      ? { scalar_preferences: props.scenario.scalar_preferences }
      : {}),
  })
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="dialog-backdrop" @click.self="emit('close')">
      <form class="dialog scenario-dialog" aria-labelledby="create-scenario-title" @submit.prevent="submit">
        <header>
          <div><span class="eyebrow">Finite-horizon comparison</span><h2 id="create-scenario-title">{{ scenario ? 'Edit scenario' : 'Create scenario' }}</h2></div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>
        <div class="field-grid">
          <label>Title<input v-model="form.title" placeholder="Reliable delivery" required /></label>
          <label>Name<input v-model="form.name" :placeholder="generatedName || 'reliable_delivery'" /></label>
        </div>
        <label>Decision context<textarea v-model="form.rationale" rows="3" placeholder="Assumptions and the decision this scenario supports"></textarea></label>
        <label>Planning horizon in periods<input v-model.number="form.planningHorizon" type="number" min="1" max="10000" step="1" required /></label>

        <fieldset class="scenario-options">
          <legend>Outcome objectives</legend>
          <label v-for="node in outcomes" :key="node.id" class="scenario-option">
            <input v-model="form.objectives[node.id]" type="checkbox" />
            <span><strong>{{ node.title }}</strong><small>{{ node.id }} · {{ node.payload.kind === 'outcome' ? node.payload.properties.direction : '' }}</small></span>
            <input v-if="form.objectives[node.id]" v-model.number="form.importance[node.id]" aria-label="Importance" type="number" min="0.000001" step="any" required />
          </label>
          <p v-if="!outcomes.length" class="form-note">Add an outcome before creating a scenario.</p>
        </fieldset>

        <fieldset class="scenario-options">
          <legend>Candidate interventions</legend>
          <label v-for="node in interventions" :key="node.id" class="scenario-option">
            <input v-model="form.candidates[node.id]" type="checkbox" />
            <span><strong>{{ node.title }}</strong><small>{{ node.id }}</small></span>
          </label>
          <p v-if="!interventions.length" class="form-note">Add an intervention before creating a scenario.</p>
        </fieldset>

        <details class="sampling-controls">
          <summary>Sampling controls</summary>
          <div class="field-grid">
            <label>Seed<input v-model.number="form.seed" type="number" min="0" step="1" required /></label>
            <label>Minimum samples<input v-model.number="form.minimumSamples" type="number" min="2" step="1" required /></label>
            <label>Maximum samples<input v-model.number="form.maximumSamples" type="number" :min="form.minimumSamples" max="10000000" step="1" required /></label>
            <label>Absolute tolerance<input v-model.number="form.absoluteTolerance" type="number" min="0" step="any" required /></label>
            <label>Relative tolerance<input v-model.number="form.relativeTolerance" type="number" min="0" step="any" required /></label>
          </div>
        </details>

        <footer>
          <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
          <button type="submit" class="primary-button" :disabled="pending || !selectedObjectives.length || !selectedCandidates.length">{{ pending ? 'Saving…' : scenario ? 'Save scenario' : 'Create scenario' }}</button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>
