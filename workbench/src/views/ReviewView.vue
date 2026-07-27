<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import BottlenecksPanel from '../components/BottlenecksPanel.vue'
import ComparisonPanel from '../components/ComparisonPanel.vue'
import MetricTimeline from '../components/MetricTimeline.vue'
import {
  useAnalysis,
  useCatalogue,
  useComparison,
  useDesign,
} from '../composables/useDesign'
import { useWorkbenchStore } from '../stores/workbench'

const props = defineProps<{ design: string; intervention?: string }>()

const router = useRouter()
const store = useWorkbenchStore()
const design = computed(() => props.design)

const { data: snapshot } = useDesign(design)
const { data: catalogue } = useCatalogue(design)

/** The variant under review: the design itself, or one of its proposals. */
const variant = computed(() => props.intervention || null)

const controls = computed(() => ({
  samples: store.samples,
  horizon: store.horizon,
  intervention: variant.value,
  series: true,
}))
const sequence = computed(() => snapshot.value?.sequence)
const { data: analysis, isFetching } = useAnalysis(design, controls, sequence)
const { data: comparison } = useComparison(design, variant, controls, sequence)

function choose(id: string | null) {
  void router.replace({
    name: 'review',
    params: { design: props.design, intervention: id ?? '' },
  })
}

/**
 * Which quantities to chart by default.
 *
 * Everything solved is too much to read at once, so the opening selection is the
 * component under the most pressure. A constraint names its component but not
 * the channels behind it — `demand` and `limit` are expressions, not references
 * — so the component is as precise as this can honestly be without the server
 * saying which channels a constraint reads.
 */
const strained = computed(() => analysis.value?.bottlenecks[0]?.component ?? null)

const watching = ref<string[]>([])

const available = computed(() => {
  const options: { value: string; component: string; channel: string }[] = []
  for (const [component, channels] of Object.entries(analysis.value?.components ?? {})) {
    for (const channel of Object.keys(channels)) {
      options.push({ value: `${component}.${channel}`, component, channel })
    }
  }
  return options
})

// Immediate, because the analysis is usually already cached from the design view
// and would otherwise never change to trigger this.
watch(
  [strained, available],
  () => {
    if (watching.value.length || !available.value.length) return
    const focused = available.value.filter((option) => option.component === strained.value)
    watching.value = (focused.length ? focused : available.value).slice(0, 4).map((o) => o.value)
  },
  { immediate: true },
)

const charts = computed(() =>
  watching.value
    .map((key) => available.value.find((option) => option.value === key))
    .filter((option): option is { value: string; component: string; channel: string } => !!option),
)

const series = computed(() => analysis.value?.series ?? [])

function unitOf(component: string, channel: string): string {
  const type = snapshot.value?.model.components.find((entry) => entry.id === component)?.type
  return (type && catalogue.value?.component_types[type]?.channels[channel]?.unit) ?? ''
}
</script>

<template>
  <div v-if="snapshot" class="review">
    <header class="variants">
      <div class="choices">
        <span class="label">Variant</span>
        <el-radio-group
          :model-value="variant ?? ''"
          size="small"
          data-test="variant-picker"
          @change="(value: string | number | boolean) => choose(String(value) || null)"
        >
          <el-radio-button value="">As designed</el-radio-button>
          <el-radio-button
            v-for="proposal in snapshot.model.interventions"
            :key="proposal.id"
            :value="proposal.id"
          >
            {{ proposal.name }}
          </el-radio-button>
        </el-radio-group>
        <el-tag v-if="isFetching" size="small" type="info">solving</el-tag>
      </div>
      <p v-if="variant" class="summary">
        {{ snapshot.model.interventions.find((entry) => entry.id === variant)?.summary }}
      </p>
    </header>

    <div class="body">
      <section class="timelines">
        <div class="picker">
          <h3>Over time</h3>
          <el-select
            v-model="watching"
            multiple
            filterable
            collapse-tags
            collapse-tags-tooltip
            placeholder="Choose quantities to watch"
            size="small"
            class="watch"
            data-test="watch-picker"
          >
            <el-option
              v-for="option in available"
              :key="option.value"
              :label="option.value"
              :value="option.value"
            />
          </el-select>
        </div>

        <el-empty
          v-if="!charts.length"
          description="Choose a quantity to watch it over time."
          :image-size="60"
        />
        <MetricTimeline
          v-for="chart in charts"
          :key="chart.value"
          :series="series"
          :component="chart.component"
          :channel="chart.channel"
          :unit="unitOf(chart.component, chart.channel)"
        />
      </section>

      <aside class="side">
        <el-tabs model-value="limits">
          <el-tab-pane label="Limits" name="limits">
            <BottlenecksPanel v-if="analysis" :analysis="analysis" :quantities="false" />
          </el-tab-pane>
          <el-tab-pane v-if="variant" label="Against baseline" name="against">
            <ComparisonPanel v-if="comparison" :comparison="comparison" />
          </el-tab-pane>
        </el-tabs>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.review { display: flex; flex-direction: column; flex: 1; min-height: 0; }
.variants {
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--line);
  background: var(--surface-strong);
}
.choices { display: flex; align-items: center; gap: var(--space-3); flex-wrap: wrap; }
.label { font-size: var(--text-2xs); text-transform: uppercase; letter-spacing: 0.04em; color: var(--muted); }
.summary { margin: var(--space-2) 0 0; font-size: var(--text-xs); color: var(--muted); max-width: 90ch; }
.body { flex: 1; display: flex; min-height: 0; }
.timelines {
  flex: 1;
  overflow: auto;
  padding: var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-width: 0;
}
.picker { display: flex; align-items: center; justify-content: space-between; gap: var(--space-4); }
.picker h3 { font-size: var(--text-md); margin: 0; }
.watch { width: 320px; }
.side {
  width: 420px;
  border-left: 1px solid var(--line);
  background: var(--surface-strong);
  overflow: auto;
  padding: 0 var(--space-4) var(--space-4);
}
</style>
