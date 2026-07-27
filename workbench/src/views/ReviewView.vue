<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import type { Intervention, Mutation } from '../api/types'
import BottlenecksPanel from '../components/BottlenecksPanel.vue'
import ComparisonPanel from '../components/ComparisonPanel.vue'
import MetricTimeline from '../components/MetricTimeline.vue'
import VariantEditor from '../components/VariantEditor.vue'
import {
  useAnalysis,
  useCatalogue,
  useComparison,
  useDesign,
  useEditDesign,
} from '../composables/useDesign'
import { readProblem } from '../domain/solverProblem'
import { useWorkbenchStore } from '../stores/workbench'

const props = defineProps<{ design: string; intervention?: string }>()

const router = useRouter()
const store = useWorkbenchStore()
const design = computed(() => props.design)

const { data: snapshot } = useDesign(design)
const { data: catalogue } = useCatalogue(design)
const edit = useEditDesign(design)

function apply(mutations: Mutation[]): Promise<unknown> {
  return edit.mutateAsync(mutations)
}

/** The variant under review: the design itself, or one of its proposals. */
const variant = computed(() => props.intervention || null)

const controls = computed(() => ({
  samples: store.samples,
  horizon: store.horizon,
  intervention: variant.value,
  series: true,
}))
const sequence = computed(() => snapshot.value?.sequence)
const { data: analysis, error: solveError, isFetching } = useAnalysis(design, controls, sequence)
const { data: comparison } = useComparison(design, variant, controls, sequence)

const problem = computed(() => readProblem(solveError.value))

function choose(id: string | null) {
  void router.replace({
    name: 'review',
    params: { design: props.design, intervention: id ?? '' },
  })
}

// Editing variants.
const editor = ref<InstanceType<typeof VariantEditor> | null>(null)
const editingVariant = ref<Intervention | null>(null)

function editVariant(entry: Intervention) {
  editingVariant.value = entry
}

function newVariant() {
  editingVariant.value = null
  editor.value?.create()
}

function removeVariant(entry: Intervention) {
  if (variant.value === entry.id) choose(null)
  void apply([{ kind: 'remove_intervention', id: entry.id }])
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
    <!--
      Variants down the side rather than across the top. There is no fixed
      number of them, they are named things somebody wrote, and the question
      being asked is "which of these" — all of which a list answers better than
      a row of buttons that wraps once there are five.
    -->
    <nav class="variants" aria-label="Variants">
      <div class="head">
        <span class="title">Variants</span>
      </div>
      <ul>
        <li>
          <button
            class="variant"
            :class="{ active: !variant }"
            data-test="variant-baseline"
            @click="choose(null)"
          >
            <el-icon class="mark"><i-document /></el-icon>
            <span class="label">As designed</span>
          </button>
        </li>
        <li v-for="entry in snapshot.model.interventions" :key="entry.id">
          <button
            class="variant"
            :class="{ active: variant === entry.id }"
            :data-test="`variant-${entry.id}`"
            @click="choose(entry.id)"
          >
            <el-icon class="mark"><i-magic-stick /></el-icon>
            <span class="label">{{ entry.name }}</span>
            <span class="actions">
              <el-icon
                class="action"
                :aria-label="`Edit ${entry.name}`"
                @click.stop="editVariant(entry)"
              >
                <i-edit-pen />
              </el-icon>
              <el-popconfirm :title="`Remove ${entry.name}?`" @confirm="removeVariant(entry)">
                <template #reference>
                  <el-icon class="action" :aria-label="`Remove ${entry.name}`" @click.stop>
                    <i-delete />
                  </el-icon>
                </template>
              </el-popconfirm>
            </span>
          </button>
        </li>
      </ul>
      <button class="variant new" data-test="new-variant" @click="newVariant">
        <el-icon class="mark"><i-plus /></el-icon>
        <span class="label">New variant</span>
      </button>
    </nav>

    <div class="main">
      <header class="context">
        <div class="what">
          <h2>{{ variant ? snapshot.model.interventions.find((e) => e.id === variant)?.name : 'As designed' }}</h2>
          <p v-if="variant" class="summary">
            {{ snapshot.model.interventions.find((entry) => entry.id === variant)?.summary }}
          </p>
          <p v-else class="summary">The design as it stands, with nothing proposed.</p>
        </div>
        <el-tag v-if="isFetching" size="small" type="info">solving</el-tag>
      </header>

      <el-alert
        v-if="problem"
        type="error"
        :closable="false"
        show-icon
        class="problem"
        data-test="review-problem"
        :title="problem.component ? `${problem.component} cannot be solved` : 'This design cannot be solved'"
        :description="problem.message"
      />

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
              popper-class="pick-watch"
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

    <VariantEditor
      ref="editor"
      :model="snapshot.model"
      :editing="editingVariant"
      :apply="apply"
      @close="editingVariant = null"
      @saved="choose"
    />
  </div>
</template>

<style scoped>
.review { display: flex; flex: 1; min-height: 0; }
.variants {
  width: 224px;
  flex: 0 0 auto;
  border-right: 1px solid var(--line);
  background: var(--surface);
  display: flex;
  flex-direction: column;
  overflow: auto;
  padding: var(--space-2);
}
.head { padding: var(--space-2) var(--space-2) var(--space-1); }
.title { font-size: var(--text-2xs); text-transform: uppercase; letter-spacing: 0.06em; color: var(--muted); font-weight: 700; }
.variants ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 1px; }
.variant {
  width: 100%;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: 6px var(--space-2);
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  text-align: left;
  font-size: var(--text-sm);
  color: var(--ink);
  min-width: 0;
}
.variant:hover { background: #e6eae2; }
.variant.active { background: var(--green-soft); color: var(--green); font-weight: 650; }
.mark { font-size: 13px; color: var(--muted); flex: 0 0 auto; }
.variant.active .mark { color: var(--green); }
.label { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.actions { display: none; gap: var(--space-1); }
.variant:hover .actions { display: flex; }
.action { font-size: 12px; color: var(--muted); }
.action:hover { color: var(--green); }
.new { margin-top: var(--space-2); color: var(--muted); }
.new:hover { color: var(--green); }

.main { flex: 1; display: flex; flex-direction: column; min-width: 0; }
.context {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--line);
  background: var(--surface-strong);
}
.what h2 { margin: 0; font-size: var(--text-lg); }
.summary { margin: 2px 0 0; font-size: var(--text-xs); color: var(--muted); max-width: 92ch; }
.problem { margin: var(--space-3) var(--space-4) 0; width: auto; }
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
.picker { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); }
.picker h3 { font-size: var(--text-md); margin: 0; white-space: nowrap; }
.watch { flex: 1; max-width: 360px; }
.side {
  width: 400px;
  flex: 0 0 auto;
  border-left: 1px solid var(--line);
  background: var(--surface-strong);
  overflow: auto;
  padding: 0 var(--space-4) var(--space-4);
}
</style>
