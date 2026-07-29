<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'

import type { Solved } from '../api/types'
import { useFlyout } from '../composables/useFlyout'
import { describeChange, directionOf, emphasisOf } from '../domain/change'
import QuantityCard from './QuantityCard.vue'

/** One quantity that can be watched, and where it came from. */
export interface SignalOption {
  value: string
  component: string
  channel: string
  family: 'input' | 'backpressure' | 'channel'
  /** What the quantity measures in, for the preview to label its axis with. */
  unit: string
  /** What the quantity is for, in the words of whoever defined it. */
  summary: string
  /**
   * Whether the component's author considers this one of its service levels.
   *
   * Only meaningful for the component's own quantities: what crossed a port is
   * sorted by which way it went rather than by how much it matters.
   */
  emphasis: 'key' | 'supporting'
}

/**
 * The bands the list can be narrowed to, in the order they are offered.
 *
 * A design of any size solves several hundred quantities, and all but a handful
 * of them are intermediate terms. Opening on the key ones alone makes the list
 * something a reader scans rather than searches, and the rest are one click
 * away for whoever is chasing a number through the arithmetic that produced it.
 */
const BANDS = [
  { key: 'key', label: 'Key metrics', hint: 'What a caller of the component experiences' },
  { key: 'supporting', label: 'Other metrics', hint: 'The workings behind the key metrics' },
  { key: 'input', label: 'Inputs', hint: 'What arrived on each inbound port' },
  { key: 'output', label: 'Outputs', hint: 'What came back from each dependency' },
] as const

type Band = (typeof BANDS)[number]['key']

/** Which band a quantity belongs to, which is the unit the filter works in. */
function bandOf(option: SignalOption): Band {
  if (option.family === 'input') return 'input'
  if (option.family === 'backpressure') return 'output'
  return option.emphasis === 'key' ? 'key' : 'supporting'
}

const props = defineProps<{
  options: SignalOption[]
  pinned: string[]
  /** Every solved quantity, so a row can be previewed without asking for it. */
  solved?: Solved
  /**
   * What each quantity settled at with and without the variant.
   *
   * Empty while the design itself is on show, because there is nothing to have
   * moved from. Given for every quantity rather than only the pinned ones, so
   * that finding what a proposal changed is a matter of reading the list rather
   * than charting things one at a time until something moves.
   */
  moved?: Record<string, { before: number; after: number }>
}>()
const emit = defineEmits<{ 'update:pinned': [string[]] }>()

const search = ref('')
const shown = ref<Band[]>(['key'])

/** The preview's width, kept in step with the card's own stylesheet. */
const PREVIEW_WIDTH = 268

/**
 * How long a pointer has to rest on a row before it is asking about it.
 *
 * A list this long is scrolled by sweeping down it, and a card that appeared
 * under every row on the way past would be a strobe rather than an answer.
 */
const DWELL_MS = 220

const previewing = ref<SignalOption | null>(null)
const row = ref<HTMLElement | null>(null)
const card = ref<InstanceType<typeof QuantityCard> | null>(null)

const { at, open, close } = useFlyout(
  () => row.value,
  () => (card.value?.$el instanceof HTMLElement ? card.value.$el : null),
  PREVIEW_WIDTH,
)

let dwell: ReturnType<typeof setTimeout> | null = null

function preview(option: SignalOption, event: Event) {
  if (dwell) clearTimeout(dwell)
  const beside = event.currentTarget
  if (!(beside instanceof HTMLElement)) return
  dwell = setTimeout(() => {
    previewing.value = option
    row.value = beside
    open()
  }, DWELL_MS)
}

function dismiss() {
  if (dwell) clearTimeout(dwell)
  previewing.value = null
  row.value = null
  close()
}

onBeforeUnmount(dismiss)

/** The solved figure behind a row, which is absent until the design is solved. */
function figureFor(option: SignalOption) {
  return props.solved?.[option.component]?.[option.channel] ?? null
}

function movement(value: string) {
  const pair = props.moved?.[value]
  if (!pair) return null
  const label = describeChange(pair.before, pair.after)
  const direction = directionOf(pair.before, pair.after)
  const emphasis = emphasisOf(pair.before, pair.after)
  return label && direction && emphasis ? { label, direction, emphasis } : null
}

/**
 * How a quantity reached its component, in the order a reader works through it.
 *
 * The component's own service levels first, because that is what somebody came
 * to look at; the workings behind them and what crossed a port are context.
 */
const BAND_ORDER: Record<Band, number> = {
  key: 0,
  supporting: 1,
  input: 2,
  output: 3,
}

/** What marks each kind of quantity, so a port reading is spotted by its shape. */
const MARKS: Record<SignalOption['family'], string> = {
  channel: 'i-plus',
  input: 'i-download',
  backpressure: 'i-upload',
}

/**
 * A quantity split into what was measured and where.
 *
 * `in.requests.rate` is the rate at the `requests` port. Written out in full it
 * repeats a prefix on every row of a group and pushes the part that differs
 * towards the end of a narrow column, so the port is carried separately.
 */
function reading(option: SignalOption): { name: string; port: string | null } {
  if (option.family === 'channel') return { name: option.channel, port: null }
  const [, port, ...measure] = option.channel.split('.')
  return { name: measure.join('.'), port: port ?? null }
}

/** Where a pinned quantity came from, once it is out of its component's group. */
function origin(option: SignalOption): string {
  const port = reading(option).port
  return port ? `${option.component} \u00b7 ${port}` : option.component
}

const byValue = computed(() => new Map(props.options.map((option) => [option.value, option])))

/**
 * The pinned quantities, in the order they were pinned.
 *
 * Ordered by the list rather than re-sorted, so the chart a reader added last is
 * where they last looked rather than somewhere alphabetical.
 */
const chosen = computed(() =>
  props.pinned.map((value) => byValue.value.get(value) ?? null).filter((option) => !!option),
)

/**
 * Solved quantities first, then alphabetically by the quantity's own name.
 *
 * A reader looking for `success_rate` knows the name before they know anything
 * else about it, and within a component that is all it takes to find.
 */
function byName(left: SignalOption, right: SignalOption): number {
  return (
    BAND_ORDER[bandOf(left)] - BAND_ORDER[bandOf(right)] ||
    left.channel.localeCompare(right.channel)
  )
}

const matching = computed(() => {
  const needle = search.value.trim().toLowerCase()
  const available = props.options.filter((option) => !props.pinned.includes(option.value))
  return needle
    ? available.filter((option) => option.value.toLowerCase().includes(needle))
    : available
})

const visible = computed(() =>
  matching.value.filter((option) => shown.value.includes(bandOf(option))),
)

/**
 * How many quantities the filter is holding back, rather than the search.
 *
 * Searching for a name and being told nothing goes by it is a lie when the row
 * exists and is merely out of band, and it is the kind of lie that has somebody
 * conclude the quantity was never solved.
 */
const withheld = computed(() => matching.value.length - visible.value.length)

/** How many of each band are on offer, so a band nothing falls in reads as empty. */
const counts = computed(() => {
  const tally = new Map<Band, number>()
  for (const option of matching.value) {
    tally.set(bandOf(option), (tally.get(bandOf(option)) ?? 0) + 1)
  }
  return tally
})

function toggle(band: Band, on: boolean) {
  shown.value = on ? [...shown.value, band] : shown.value.filter((entry) => entry !== band)
}

/**
 * One group per component, because that is the unit somebody reasons about.
 *
 * A design's quantities all have the same few names, so a list headed by the
 * name asks a reader to pick their component out of a dozen identical rows.
 */
const groups = computed(() => {
  const held = new Map<string, SignalOption[]>()
  for (const option of visible.value) {
    const kept = held.get(option.component)
    if (kept) kept.push(option)
    else held.set(option.component, [option])
  }
  return [...held.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([component, options]) => ({ label: component, options: [...options].sort(byName) }))
})

function pin(value: string) {
  // The row is about to move to the watched list, taking its anchor with it.
  dismiss()
  if (props.pinned.includes(value)) return
  emit('update:pinned', [...props.pinned, value])
}

function unpin(value: string) {
  dismiss()
  emit(
    'update:pinned',
    props.pinned.filter((entry) => entry !== value),
  )
}
</script>

<template>
  <section class="signals" data-test="watch-picker">
    <div class="head">
      <span class="title">Watching</span>
      <button
        v-if="pinned.length"
        class="clear"
        data-test="clear-signals"
        @click="emit('update:pinned', [])"
      >
        clear
      </button>
    </div>

    <ul v-if="chosen.length" class="pinned">
      <li v-for="option in chosen" :key="option.value">
        <button
          class="signal chosen"
          :data-test="`unpin-${option.value}`"
          :aria-label="`Stop watching ${option.value}`"
          @click="unpin(option.value)"
          @mouseenter="preview(option, $event)"
          @mouseleave="dismiss"
          @focus="preview(option, $event)"
          @blur="dismiss"
        >
          <el-icon class="mark"><i-view /></el-icon>
          <span class="name">
            <span class="channel">{{ reading(option).name }}</span>
            <span class="component">{{ origin(option) }}</span>
          </span>
          <span
            v-if="movement(option.value)"
            class="moved"
            :class="[movement(option.value)!.direction, movement(option.value)!.emphasis]"
          >
            {{ movement(option.value)!.label }}
          </span>
          <el-icon class="act"><i-close /></el-icon>
        </button>
      </li>
    </ul>
    <p v-else class="none">Nothing charted yet. Pick a quantity below.</p>

    <div class="sift">
      <el-input
        v-model="search"
        size="small"
        placeholder="Find a quantity"
        clearable
        class="find"
        data-test="signal-search"
      >
        <template #prefix>
          <el-icon><i-search /></el-icon>
        </template>
      </el-input>

      <!--
        Behind a button because the bands are a setting rather than a question:
        four permanent checkboxes above a list in a rail this narrow cost more
        room than the list they were filtering.
      -->
      <el-popover placement="bottom-end" trigger="click" :width="248" popper-class="bands">
        <template #reference>
          <button
            class="sieve"
            :class="{ narrowed: shown.length !== BANDS.length }"
            aria-label="Choose which quantities are listed"
            data-test="signal-bands"
          >
            <el-icon :size="14"><i-filter /></el-icon>
          </button>
        </template>

        <p class="band-title">Show</p>
        <label v-for="band in BANDS" :key="band.key" class="band">
          <el-checkbox
            :model-value="shown.includes(band.key)"
            size="small"
            :data-test="`band-${band.key}`"
            @update:model-value="(on: string | number | boolean) => toggle(band.key, on === true)"
          />
          <span class="band-label">
            <span class="band-name">
              {{ band.label }}
              <span class="band-count">{{ counts.get(band.key) ?? 0 }}</span>
            </span>
            <span class="band-hint">{{ band.hint }}</span>
          </span>
        </label>
      </el-popover>
    </div>

    <div class="list">
      <template v-for="group in groups" :key="group.label">
        <p class="group">{{ group.label }}</p>
        <button
          v-for="option in group.options"
          :key="option.value"
          class="signal"
          :class="[option.family, bandOf(option)]"
          :data-test="`pin-${option.value}`"
          :aria-label="`Watch ${option.value}`"
          @click="pin(option.value)"
          @mouseenter="preview(option, $event)"
          @mouseleave="dismiss"
          @focus="preview(option, $event)"
          @blur="dismiss"
        >
          <el-icon class="mark"><component :is="MARKS[option.family]" /></el-icon>
          <span class="name flat">
            <span class="channel">{{ reading(option).name }}</span>
            <span v-if="reading(option).port" class="port">{{ reading(option).port }}</span>
          </span>
          <span
            v-if="movement(option.value)"
            class="moved"
            :class="[movement(option.value)!.direction, movement(option.value)!.emphasis]"
          >
            {{ movement(option.value)!.label }}
          </span>
        </button>
      </template>
      <p v-if="!groups.length && !withheld" class="none">
        {{ options.length ? 'Nothing goes by that name.' : 'Solve the design to see its quantities.' }}
      </p>
      <p v-if="withheld" class="withheld" data-test="signals-withheld">
        {{ withheld }} more hidden by the filter.
        <button class="reveal" data-test="show-all-bands" @click="shown = BANDS.map((b) => b.key)">
          show everything
        </button>
      </p>
    </div>

    <!--
      Rendered at the document root rather than beside the row it belongs to.
      This rail scrolls, and a scrolling box crops whatever is positioned inside
      it — which is every pixel of a card whose whole purpose is to hang outside,
      where there is room for a chart.
    -->
    <Teleport v-if="at && previewing" to="body">
      <QuantityCard
        ref="card"
        class="flyout"
        :style="{ left: `${at.left}px`, top: `${at.top}px` }"
        :heading="previewing.component"
        :subject="previewing.channel"
        :summary="previewing.summary"
        :unit="previewing.unit"
        :quantity="figureFor(previewing)"
      />
    </Teleport>
  </section>
</template>

<style scoped>
.signals {
  display: flex;
  flex-direction: column;
  min-height: 0;
  flex: 1;
  border-top: 1px solid var(--line);
  margin-top: var(--space-2);
  padding-top: var(--space-2);
}
.head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  padding: var(--space-1) var(--space-2);
}
.title {
  font-family: var(--display);
  font-size: var(--text-2xs);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--muted);
  font-weight: 700;
}
.clear {
  border: none;
  background: none;
  padding: 0;
  font-size: var(--text-2xs);
  color: var(--muted);
  text-decoration: underline;
}
.clear:hover { color: var(--green); }

.pinned { list-style: none; margin: 0 0 var(--space-2); padding: 0; display: flex; flex-direction: column; gap: 1px; }
.none { margin: 0 var(--space-2) var(--space-2); font-size: var(--text-2xs); color: var(--muted); }

.signal {
  width: 100%;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: 5px var(--space-2);
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  text-align: left;
  color: var(--ink);
  min-width: 0;
}
.signal:hover { background: #e6eae2; }
.signal.chosen { background: var(--green-soft); }
.signal.chosen .mark { color: var(--green); }
.mark { font-size: 12px; color: var(--muted); flex: 0 0 auto; }
.name { display: flex; flex-direction: column; min-width: 0; flex: 1; line-height: 1.25; }
/* Grouped by component, so a row has room to read on one line. */
.name.flat { flex-direction: row; align-items: baseline; gap: 6px; }
.channel {
  font-family: var(--mono);
  font-size: var(--text-2xs);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.component { font-size: 10px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.port {
  font-family: var(--mono);
  font-size: 10px;
  color: var(--muted);
  /* Never truncated: a two-letter stub of a port name says less than nothing. */
  flex: 0 0 auto;
  white-space: nowrap;
}
.signal.input .mark { color: #5b8d7c; }
.signal.backpressure .mark { color: #a08a56; }
/* A service level, so it reads as the row somebody came here for. */
.signal.key .channel { font-weight: 600; }
.signal.supporting .channel { color: var(--muted); }
/*
 * Which way it went, not whether that is good news. Higher latency and higher
 * throughput are the same arrow, and only the reader knows which they wanted.
 *
 * Colour therefore says how far it moved rather than taking a side: the house
 * accent marks the handful worth opening, and everything else stays a quiet
 * outline so a list where every row moved a little still reads as calm.
 */
.moved {
  font-family: var(--mono);
  font-size: 10px;
  line-height: 1;
  padding: 3px 6px;
  border-radius: 999px;
  border: 1px solid var(--line);
  color: var(--muted);
  background: var(--surface);
  flex: 0 0 auto;
}
.moved.notable {
  border-color: #b7d0c3;
  background: var(--green-soft);
  color: var(--green);
  font-weight: 600;
}
.signal.chosen .moved.notable { background: var(--surface-strong); }
.signal.chosen:hover .moved { display: none; }
.act { font-size: 11px; color: var(--muted); flex: 0 0 auto; opacity: 0; }
.signal:hover .act { opacity: 1; }

.find { margin: 0 var(--space-2) var(--space-2); width: auto; }

.sift {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  margin: 0 var(--space-2) var(--space-2);
}
.sift .find { margin: 0; flex: 1; min-width: 0; }
.sieve {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--surface);
  color: var(--muted);
}
.sieve:hover { color: var(--ink); }
/* A list that is not showing everything says so, rather than looking complete. */
.sieve.narrowed { color: var(--green); border-color: #b7d0c3; background: var(--green-soft); }

.withheld {
  margin: var(--space-1) var(--space-2) var(--space-2);
  font-size: var(--text-2xs);
  color: var(--muted);
}
.reveal {
  border: none;
  background: none;
  padding: 0;
  font-size: var(--text-2xs);
  color: var(--muted);
  text-decoration: underline;
}
.reveal:hover { color: var(--green); }

.list { flex: 1; overflow: auto; min-height: 0; }
/* A component name, so it is written the way the design writes it. */
.group {
  position: sticky;
  top: 0;
  z-index: 1;
  margin: var(--space-2) 0 2px;
  padding: 3px var(--space-2);
  background: var(--surface);
  font-family: var(--mono);
  font-size: var(--text-2xs);
  font-weight: 600;
  color: var(--ink);
  border-bottom: 1px solid var(--line);
}
/* Above dialogs, because a design can be reviewed with one open. */
.flyout { position: fixed; z-index: 2100; }
</style>

<style>
/* The popover renders at the body, so its contents cannot be scoped. */
.bands .band-title {
  margin: 0 0 var(--space-2);
  font-family: var(--display);
  font-size: var(--text-2xs);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--muted);
  font-weight: 700;
}
.bands .band { display: flex; align-items: flex-start; gap: var(--space-2); padding: var(--space-1) 0; }
.bands .band + .band { border-top: 1px solid var(--line); }
.bands .band-label { min-width: 0; display: flex; flex-direction: column; }
.bands .band-name { font-size: var(--text-sm); font-weight: 650; }
.bands .band-count { font-family: var(--mono); font-size: var(--text-2xs); color: var(--muted); margin-left: 4px; }
.bands .band-hint { margin-top: 2px; font-size: var(--text-2xs); color: var(--muted); line-height: 1.4; }
.bands .el-checkbox { height: auto; margin-top: 2px; }
.bands .el-checkbox__label { display: none; }
</style>
