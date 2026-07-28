<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import type { Intervention, Mutation } from '../api/types'
import BottlenecksPanel from '../components/BottlenecksPanel.vue'
import ChartSkeleton from '../components/ChartSkeleton.vue'
import ComparisonPanel from '../components/ComparisonPanel.vue'
import MetricTimeline from '../components/MetricTimeline.vue'
import SignalPicker, { type SignalOption } from '../components/SignalPicker.vue'
import SkeletonBlock from '../components/SkeletonBlock.vue'
import SolveProgress from '../components/SolveProgress.vue'
import SolvingVeil from '../components/SolvingVeil.vue'
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
  step: store.step,
  transient: store.transient,
  intervention: variant.value,
  series: true,
}))
const sequence = computed(() => snapshot.value?.sequence)

const { data: analysis, error: solveError, isFetching } = useAnalysis(design, controls, sequence)
const { data: comparison, isFetching: comparing } = useComparison(
  design,
  variant,
  controls,
  sequence,
)

/**
 * The design as it stands, solved alongside whichever variant is on show.
 *
 * A second request rather than a field on the first, because it is a second
 * scenario: the server solves the two in parallel and remembers both, so the
 * baseline is computed once however many variants are looked at afterwards.
 * Skipped entirely while the baseline is what is being looked at, since a chart
 * comparing something with itself is a flat line and a wasted solve.
 */
const baselineControls = computed(() => ({ ...controls.value, intervention: null }))
const { data: baseline, isFetching: solvingBaseline } = useAnalysis(
  () => (variant.value ? design.value : null),
  baselineControls,
  sequence,
)

/**
 * What makes two solves cost the same amount of arithmetic.
 *
 * The variant is left out on purpose: rebinding a shared quantity changes the
 * answer without changing the work, so the first variant solved is what teaches
 * the progress bar how long the rest will take.
 */
const shape = computed(() =>
  [design.value, store.samples, store.horizon, store.step, store.transient].join('/'),
)

/**
 * Whether there is nothing yet to draw.
 *
 * Distinct from fetching. A re-solve keeps the previous answer mounted because
 * throwing away a nearly-right chart makes the page flash; a first solve has
 * nothing to keep, and an outline of what is coming says more than an empty
 * panel does.
 */
const awaitingFirstAnswer = computed(() => !analysis.value && isFetching.value)

/**
 * Whether what is on screen is about the question being asked.
 *
 * The charts below are kept while the next answer is solved, so for a moment
 * they show one variant under another's name. That is the one failure this view
 * must not have — a reader who takes those numbers for the variant they just
 * chose draws a conclusion about a design nobody solved — so anything retained
 * is covered until the answer catches up.
 */
const stale = computed(
  () => !awaitingFirstAnswer.value && (isFetching.value || solvingBaseline.value),
)

/** What the veil says it is working on, which is the thing the reader just asked for. */
const solvingLabel = computed(() =>
  variant.value
    ? `Solving ${snapshot.value?.model.interventions.find((e) => e.id === variant.value)?.name ?? variant.value}`
    : 'Solving as designed',
)

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
 * Everything solved is too much to read at once, so the opening selection is
 * what a user of the system would have experienced: the latency and success a
 * caller observed. Responses travel back to whoever made the call, so those two
 * figures already account for every hop, retry, timeout and fan-out behind them,
 * which makes them the only numbers that answer "is this design good enough"
 * without further assembly.
 *
 * Falling back to the most pressured component keeps a design with no callers
 * readable. A constraint names its component but not the channels behind it —
 * `demand` and `limit` are expressions, not references — so the component is as
 * precise as that fallback can honestly be.
 */
const callers = computed(() => {
  const types = catalogue.value?.component_types ?? {}
  return (snapshot.value?.model.components ?? [])
    .filter((component) => Object.keys(types[component.type]?.ports?.in ?? {}).length === 0)
    .map((component) => component.id)
})

const strained = computed(() => analysis.value?.bottlenecks[0]?.component ?? null)

const watching = ref<string[]>([])

/** How a quantity reached the component, for grouping the picker. */
function family(channel: string): 'input' | 'backpressure' | 'channel' {
  if (channel.startsWith('in.')) return 'input'
  if (channel.startsWith('out.')) return 'backpressure'
  return 'channel'
}

const available = computed(() => {
  const options: SignalOption[] = []
  for (const [component, channels] of Object.entries(analysis.value?.components ?? {})) {
    for (const channel of Object.keys(channels)) {
      options.push({ value: `${component}.${channel}`, component, channel, family: family(channel) })
    }
  }
  return options
})

/**
 * The service levels a caller actually observed.
 *
 * Success and failure are one fact stated twice — `failure` is the offered rate
 * times one minus `success` — so charting both spends a second panel restating
 * the first upside down. The share that succeeded is the form an objective is
 * written in, and being dimensionless it renders as a percentage on its own.
 */
const SERVICE_LEVELS = ['success', 'latency']

/** How a quantity reads when its channel name is not the clearest label. */
const LABELS: Record<string, string> = {
  success: 'Success rate',
  latency: 'Response time',
}

function labelFor(channel: string): string {
  return LABELS[channel] ?? ''
}

watch(
  [callers, strained, available],
  () => {
    if (watching.value.length || !available.value.length) return
    const observed = callers.value.flatMap((caller) =>
      SERVICE_LEVELS.map((channel) => `${caller}.${channel}`).filter((key) =>
        available.value.some((option) => option.value === key),
      ),
    )
    if (observed.length) {
      watching.value = observed.slice(0, 4)
      return
    }
    const focused = available.value.filter((option) => option.component === strained.value)
    watching.value = (focused.length ? focused : available.value).slice(0, 4).map((o) => o.value)
  },
  { immediate: true },
)

const charts = computed(() =>
  watching.value
    .map((key) => available.value.find((option) => option.value === key))
    .filter((option) => !!option),
)

const series = computed(() => analysis.value?.series ?? [])

/** The same steps in the design as it stands, where a variant is on show. */
const baselineSeries = computed(() => (variant.value ? (baseline.value?.series ?? []) : []))

/** What the baseline is called, which is what the sidebar calls it. */
const BASELINE_LABEL = 'as designed'

/**
 * The unit a quantity carries.
 *
 * A port signal is named for the signal it carries rather than a channel, so its
 * unit comes from the signal vocabulary; anything else is one of the component's
 * own channels.
 */
function unitOf(component: string, channel: string): string {
  const type = snapshot.value?.model.components.find((entry) => entry.id === component)?.type
  if (channel.includes('.')) {
    const signal = channel.slice(channel.lastIndexOf('.') + 1)
    return catalogue.value?.signals?.[signal]?.unit ?? ''
  }
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
    <nav class="rail" aria-label="Variants and quantities">
      <div class="variants">
        <div class="head">
          <span class="title">Variants</span>
          <button class="add" data-test="new-variant" title="New variant" @click="newVariant">
            <el-icon><i-plus /></el-icon>
          </button>
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
              :title="entry.name"
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
      </div>

      <!--
        The quantities on show live beside the variants rather than in a dropdown
        above the charts. Choosing what to watch and choosing what to watch it
        against are the same act of framing a question, and a menu that closed
        after every pick made assembling a set of four an exercise in patience.
      -->
      <SignalPicker
        :options="available"
        :pinned="watching"
        @update:pinned="(next) => (watching = next)"
      />
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
        <SolveProgress :solving="isFetching" :shape="shape" />
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
            <span v-if="store.transient" class="mode" data-test="through-time">through time</span>
            <span v-if="variant" class="against">
              compared with <strong>{{ BASELINE_LABEL }}</strong>
            </span>
          </div>

          <template v-if="awaitingFirstAnswer">
            <ChartSkeleton v-for="slot in 2" :key="slot" />
          </template>
          <el-empty
            v-else-if="!charts.length"
            description="Pick a quantity in the sidebar to watch it over time."
            :image-size="60"
          />
          <SolvingVeil v-else :busy="stale" :label="solvingLabel">
            <MetricTimeline
              v-for="chart in charts"
              :key="chart.value"
              :series="series"
              :baseline="baselineSeries"
              :baseline-label="BASELINE_LABEL"
              :component="chart.component"
              :channel="chart.channel"
              :label="labelFor(chart.channel)"
              :unit="unitOf(chart.component, chart.channel)"
            />
          </SolvingVeil>
        </section>

        <aside class="side">
          <el-tabs model-value="limits">
            <el-tab-pane label="Limits" name="limits">
              <SolvingVeil v-if="analysis" :busy="isFetching" :label="solvingLabel">
                <BottlenecksPanel :analysis="analysis" :quantities="false" />
              </SolvingVeil>
              <div v-else-if="isFetching" class="rows" aria-busy="true">
                <SkeletonBlock v-for="row in 5" :key="row" height="15px" />
              </div>
            </el-tab-pane>
            <el-tab-pane v-if="variant" label="Against baseline" name="against">
              <SolvingVeil v-if="comparison" :busy="comparing" :label="solvingLabel">
                <ComparisonPanel :comparison="comparison" />
              </SolvingVeil>
              <div v-else-if="comparing" class="rows" aria-busy="true">
                <SkeletonBlock v-for="row in 5" :key="row" height="15px" />
              </div>
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
/*
 * Both rails give way to the charts as the window narrows. They are navigation
 * and reference; the charts are the thing being read, and a fixed pair of rails
 * left a laptop with a column too narrow to caption.
 */
.rail {
  width: clamp(196px, 22vw, 236px);
  flex: 0 0 auto;
  border-right: 1px solid var(--line);
  background: var(--surface);
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: var(--space-2);
}
/* Capped so a design with thirty variants cannot squeeze the picker away. */
.variants { display: flex; flex-direction: column; min-height: 0; max-height: 50%; overflow: auto; }
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-1) var(--space-2);
}
.title { font-size: var(--text-2xs); text-transform: uppercase; letter-spacing: 0.06em; color: var(--muted); font-weight: 700; }
.add {
  display: inline-flex;
  border: none;
  background: none;
  padding: 2px;
  border-radius: var(--radius-sm);
  color: var(--muted);
  font-size: 13px;
}
.add:hover { background: var(--green-soft); color: var(--green); }
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
.picker { display: flex; align-items: baseline; gap: var(--space-2); }
.picker h3 { font-size: var(--text-md); margin: 0; white-space: nowrap; }
.mode {
  font-size: var(--text-2xs);
  color: var(--green);
  background: var(--green-soft);
  border-radius: 999px;
  padding: 1px var(--space-2);
}
.against { margin-left: auto; font-size: var(--text-2xs); color: var(--muted); }
.against strong { font-weight: 650; color: var(--ink); }
.side {
  width: clamp(256px, 28vw, 344px);
  flex: 0 0 auto;
  border-left: 1px solid var(--line);
  background: var(--surface-strong);
  overflow: auto;
  padding: 0 var(--space-4) var(--space-4);
}
.rows { display: flex; flex-direction: column; gap: var(--space-2); padding-top: var(--space-2); }
</style>
