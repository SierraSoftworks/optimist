<script setup lang="ts">
import { computed, watch } from 'vue'

import type { Mutation } from './api/types'
import BottlenecksPanel from './components/BottlenecksPanel.vue'
import ComparisonPanel from './components/ComparisonPanel.vue'
import DesignPanel from './components/DesignPanel.vue'
import {
  useAnalysis,
  useCatalogue,
  useComparison,
  useDesign,
  useDesigns,
  useEditDesign,
} from './composables/useDesign'
import { useWorkbenchStore } from './stores/workbench'

const store = useWorkbenchStore()

const designs = useDesigns()
const design = computed(() => store.design)
const { data: snapshot, feedStatus, error: designError } = useDesign(design)
const { data: catalogue } = useCatalogue(design)

const controls = computed(() => ({ samples: store.samples, horizon: store.horizon }))
const sequence = computed(() => snapshot.value?.sequence)
const { data: analysis, error: analysisError, isFetching } = useAnalysis(design, controls, sequence)
const intervention = computed(() => store.intervention)
const { data: comparison, error: comparisonError } = useComparison(
  design,
  intervention,
  controls,
  sequence,
)

const edit = useEditDesign(design)

// Open the first design so the tool is useful on arrival rather than showing an
// empty frame and a picker.
watch(
  () => designs.data.value,
  (available) => {
    if (!store.design && available?.length) store.open(available[0].id)
  },
  { immediate: true },
)

const failure = computed(
  () => designError.value ?? analysisError.value ?? comparisonError.value ?? edit.error.value,
)

function apply(mutations: Mutation[]) {
  edit.mutate(mutations)
}
</script>

<template>
  <div class="workbench">
    <header>
      <div class="brand">
        <strong>Optimist</strong>
        <span class="feed" :class="feedStatus">{{ feedStatus }}</span>
      </div>

      <label class="picker">
        <span class="sr-only">Design</span>
        <select :value="store.design ?? ''" @change="store.open(($event.target as HTMLSelectElement).value)">
          <option v-for="entry in designs.data.value ?? []" :key="entry.id" :value="entry.id">
            {{ entry.name }}
          </option>
        </select>
      </label>

      <nav class="views">
        <button
          v-for="view in (['design', 'bottlenecks', 'compare'] as const)"
          :key="view"
          type="button"
          :class="{ active: store.view === view }"
          :aria-pressed="store.view === view"
          @click="store.view = view"
        >
          {{ view }}
        </button>
      </nav>

      <label class="control">
        <span>samples</span>
        <input v-model.number="store.samples" type="number" min="64" max="20000" step="500" />
      </label>
      <label class="control">
        <span>horizon</span>
        <input v-model.number="store.horizon" type="number" min="1" max="500" />
      </label>

      <span v-if="isFetching" class="solving">solving</span>
    </header>

    <p v-if="failure" class="failure" role="alert">
      {{ failure.message }}
      <span v-for="line in (failure as { advice?: string[] }).advice ?? []" :key="line" class="advice">
        {{ line }}
      </span>
    </p>

    <main>
      <template v-if="snapshot">
        <div class="title">
          <h1>{{ snapshot.name }}</h1>
          <p>{{ snapshot.summary }}</p>
        </div>

        <DesignPanel
          v-if="store.view === 'design'"
          :model="snapshot.model"
          :catalogue="catalogue"
          @edit="apply"
        />

        <BottlenecksPanel v-else-if="store.view === 'bottlenecks' && analysis" :analysis="analysis" />

        <template v-else-if="store.view === 'compare'">
          <label class="picker wide">
            <span class="sr-only">Proposal</span>
            <select
              :value="store.intervention ?? ''"
              @change="store.intervention = ($event.target as HTMLSelectElement).value || null"
            >
              <option value="">Choose a proposal&hellip;</option>
              <option v-for="entry in snapshot.model.interventions" :key="entry.id" :value="entry.id">
                {{ entry.name }}
              </option>
            </select>
          </label>
          <p v-if="store.intervention" class="hint">
            {{ snapshot.model.interventions.find((i) => i.id === store.intervention)?.summary }}
          </p>
          <ComparisonPanel v-if="comparison" :comparison="comparison" />
        </template>

        <p v-else class="empty">Solving&hellip;</p>
      </template>

      <p v-else-if="!designs.data.value?.length" class="empty">
        This server is not holding any designs.
      </p>
    </main>
  </div>
</template>

<style scoped>
.workbench { display: flex; flex-direction: column; height: 100vh; }
header {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  padding: var(--space-2) var(--space-4);
  border-bottom: 1px solid var(--line);
  background: var(--surface-strong);
  flex-wrap: wrap;
}
.brand { display: flex; align-items: baseline; gap: var(--space-2); }
.feed { font-size: var(--text-2xs); font-family: var(--mono); color: var(--muted); }
.feed.open { color: var(--green); }
.feed.closed { color: var(--danger); }
select, input { border: 1px solid var(--line); border-radius: var(--radius-sm); padding: 4px 7px; background: var(--surface); }
.views { display: flex; gap: 2px; }
.views button {
  border: 1px solid transparent;
  background: none;
  border-radius: var(--radius-sm);
  padding: 4px 10px;
  font-size: var(--text-sm);
  text-transform: capitalize;
}
.views button.active { background: var(--green-soft); border-color: var(--green); color: var(--green); font-weight: 650; }
.control { display: flex; align-items: center; gap: var(--space-1); font-size: var(--text-2xs); color: var(--muted); }
.control input { width: 7ch; font-family: var(--mono); }
.solving { font-size: var(--text-2xs); color: var(--muted); font-family: var(--mono); }
main { flex: 1; overflow: auto; padding: var(--space-5); max-width: var(--measure); width: 100%; }
.title h1 { font-size: var(--text-2xl); margin: 0; }
.title p { color: var(--muted); font-size: var(--text-sm); margin: var(--space-1) 0 var(--space-5); max-width: 70ch; }
.failure {
  margin: 0;
  padding: var(--space-3) var(--space-4);
  background: var(--danger-surface);
  border-bottom: 1px solid var(--danger-line);
  color: var(--danger);
  font-size: var(--text-sm);
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.advice { font-size: var(--text-xs); opacity: 0.85; }
.picker.wide { display: block; margin-bottom: var(--space-2); }
.hint { color: var(--muted); font-size: var(--text-xs); margin: 0 0 var(--space-4); max-width: 70ch; }
.empty { color: var(--muted); font-size: var(--text-sm); }
</style>
