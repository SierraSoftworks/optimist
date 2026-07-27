<script setup lang="ts">
import { computed, ref } from 'vue'

import type { Frame, Quantity } from '../api/types'
import { formatSiNumber } from '../domain/humanNumber'
import { kernelDensity } from '../domain/density'

const props = withDefaults(
  defineProps<{
    series: Frame[]
    component: string
    channel: string
    unit?: string
    height?: number
  }>(),
  { unit: '', height: 120 },
)

const WIDTH = 640
const PADDING = { left: 46, right: 8, top: 10, bottom: 20 }

/** The quantity at each step, dropping steps where this channel is absent. */
const points = computed(() =>
  props.series
    .map((frame) => ({
      time: frame.time,
      quantity: frame.components[props.component]?.[props.channel] as Quantity | undefined,
    }))
    .filter((point): point is { time: number; quantity: Quantity } => point.quantity !== undefined),
)

/**
 * Vertical range covering the whole band, not just the medians.
 *
 * Scaling to the median would clip the spread that the band exists to show, and
 * a reader would see a confident line where the model is anything but.
 */
const bounds = computed(() => {
  const values = points.value.flatMap((point) => [point.quantity.p10, point.quantity.p90])
  if (!values.length) return { low: 0, high: 1 }
  const low = Math.min(...values)
  const high = Math.max(...values)
  if (high === low) return { low: low - 0.5, high: high + 0.5 }
  const margin = (high - low) * 0.08
  return {
    // Nothing solved here is negative — rates, times, occupancies and shares are
    // all non-negative — so the margin is not allowed to push the axis below
    // zero and label a probability as minus eight percent.
    low: low >= 0 ? Math.max(0, low - margin) : low - margin,
    high: high + margin,
  }
})

const plot = { width: WIDTH - PADDING.left - PADDING.right, height: props.height - PADDING.top - PADDING.bottom }

function x(index: number): number {
  const count = points.value.length
  const step = count > 1 ? plot.width / (count - 1) : 0
  return PADDING.left + index * step
}

function y(value: number): number {
  const { low, high } = bounds.value
  const fraction = (value - low) / (high - low || 1)
  return PADDING.top + plot.height - fraction * plot.height
}

const median = computed(() =>
  points.value.map((point, index) => `${x(index)},${y(point.quantity.p50)}`).join(' '),
)

/** The p10–p90 band, drawn as one closed path down the upper edge and back. */
const band = computed(() => {
  if (points.value.length < 2) return ''
  const upper = points.value.map((point, index) => `${x(index)},${y(point.quantity.p90)}`)
  const lower = points.value
    .map((point, index) => `${x(index)},${y(point.quantity.p10)}`)
    .reverse()
  return `M${upper.join(' L')} L${lower.join(' L')} Z`
})

const hovered = ref<number | null>(null)
const active = computed(() =>
  hovered.value === null ? null : (points.value[hovered.value] ?? null),
)

/**
 * The nearest step to the pointer.
 *
 * Snapping to a step rather than interpolating is deliberate: there is no value
 * between two steps, and drawing one would invent a figure the solver never
 * produced.
 */
function track(event: MouseEvent) {
  const target = event.currentTarget as SVGSVGElement
  const box = target.getBoundingClientRect()
  const position = ((event.clientX - box.left) / box.width) * WIDTH
  const count = points.value.length
  if (count === 0) return
  const step = count > 1 ? plot.width / (count - 1) : 1
  const index = Math.round((position - PADDING.left) / step)
  hovered.value = Math.max(0, Math.min(count - 1, index))
}

/** The hovered step's distribution, as a small density sketch. */
const sketch = computed(() => {
  const quantity = active.value?.quantity
  if (!quantity || quantity.draws.length === 0) return null
  const density = kernelDensity(quantity.draws)
  if (!density) return null
  const from = density.x[0]
  const to = density.x[density.x.length - 1]
  const span = to - from || 1
  const tallest = Math.max(...density.y) || 1
  const w = 150
  const h = 44
  const path = density.x
    .map((value, index) => `${((value - from) / span) * w},${h - (density.y[index] / tallest) * h}`)
    .join(' L')
  return { path: `M0,${h} L${path} L${w},${h} Z`, modes: density.modes, width: w, height: h }
})

const ticks = computed(() => {
  const { low, high } = bounds.value
  return [low, (low + high) / 2, high].map((value) => ({ value, y: y(value) }))
})
</script>

<template>
  <figure class="timeline">
    <figcaption>
      <span class="channel">{{ channel }}</span>
      <span class="component">{{ component }}</span>
      <span v-if="unit" class="unit">{{ unit }}</span>
    </figcaption>

    <svg
      :viewBox="`0 0 ${WIDTH} ${height}`"
      class="plot"
      role="img"
      :aria-label="`${channel} of ${component} over time`"
      @mousemove="track"
      @mouseleave="hovered = null"
    >
      <line
        v-for="tick in ticks"
        :key="tick.value"
        class="gridline"
        :x1="PADDING.left"
        :x2="WIDTH - PADDING.right"
        :y1="tick.y"
        :y2="tick.y"
      />
      <text
        v-for="tick in ticks"
        :key="`label-${tick.value}`"
        class="tick"
        :x="PADDING.left - 6"
        :y="tick.y + 3"
        text-anchor="end"
      >
        {{ formatSiNumber(tick.value) }}
      </text>

      <path v-if="band" class="band" :d="band" />
      <polyline v-if="median" class="median" :points="median" />

      <template v-if="hovered !== null && active">
        <line
          class="cursor"
          :x1="x(hovered)"
          :x2="x(hovered)"
          :y1="PADDING.top"
          :y2="PADDING.top + plot.height"
        />
        <circle class="dot" :cx="x(hovered)" :cy="y(active.quantity.p50)" r="3" />
      </template>
    </svg>

    <!--
      The hovered step's own distribution. A line chart of medians hides whether
      a value is one outcome or two, which for a design near a fold is the thing
      being looked for, so stopping on a point shows the shape behind it.
    -->
    <div v-if="active" class="readout">
      <div class="numbers">
        <span class="time">t = {{ formatSiNumber(active.time) }}s</span>
        <span class="value">{{ formatSiNumber(active.quantity.p50) }}</span>
        <span class="range">
          {{ formatSiNumber(active.quantity.p10) }} &ndash;
          {{ formatSiNumber(active.quantity.p90) }}
        </span>
      </div>
      <svg v-if="sketch" :viewBox="`0 0 ${sketch.width} ${sketch.height}`" class="sketch">
        <path :d="sketch.path" />
      </svg>
      <el-tag v-if="sketch && sketch.modes > 1" type="warning" size="small" effect="light">
        {{ sketch.modes }} states
      </el-tag>
      <span v-else-if="!sketch" class="certain">certain</span>
    </div>
    <div v-else class="readout placeholder">Hover to read a step.</div>
  </figure>
</template>

<style scoped>
.timeline {
  margin: 0;
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface-strong);
  padding: var(--space-3);
}
figcaption { display: flex; align-items: baseline; gap: var(--space-2); margin-bottom: var(--space-1); }
.channel { font-family: var(--mono); font-size: var(--text-sm); font-weight: 650; }
.component { font-size: var(--text-2xs); color: var(--muted); }
.unit { font-family: var(--mono); font-size: var(--text-2xs); color: var(--muted); margin-left: auto; }
.plot { width: 100%; display: block; cursor: crosshair; }
.gridline { stroke: var(--line); stroke-width: 1; }
.tick { font-family: var(--mono); font-size: 9px; fill: var(--muted); }
.band { fill: var(--green); fill-opacity: 0.16; }
.median { fill: none; stroke: var(--green); stroke-width: 1.75; stroke-linejoin: round; }
.cursor { stroke: var(--ink); stroke-width: 1; stroke-dasharray: 2 2; }
.dot { fill: var(--green); stroke: var(--surface-strong); stroke-width: 1.5; }
.readout {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-height: 52px;
  border-top: 1px solid var(--line);
  padding-top: var(--space-2);
}
.readout.placeholder { color: var(--muted); font-size: var(--text-xs); font-style: italic; }
.numbers { display: flex; flex-direction: column; font-family: var(--mono); }
.time { font-size: var(--text-2xs); color: var(--muted); }
.value { font-size: var(--text-lg); }
.range { font-size: var(--text-2xs); color: var(--muted); }
.sketch { width: 150px; height: 44px; }
.sketch path { fill: var(--green); fill-opacity: 0.45; stroke: var(--green); stroke-width: 1.25; }
.certain { font-size: var(--text-2xs); color: var(--muted); }
</style>
