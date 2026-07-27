<script setup lang="ts">
import { computed, ref } from 'vue'

import type { Frame, Quantity } from '../api/types'
import { kernelDensity } from '../domain/density'
import { formatSiNumber } from '../domain/humanNumber'
import { scaleFor, showScaled, showWithUnit } from '../domain/units'

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
const PADDING = { left: 52, right: 10, top: 10, bottom: 20 }

/** The quantity at each step, dropping steps where this channel is absent. */
const points = computed(() =>
  props.series
    .map((frame) => ({
      time: frame.time,
      quantity: frame.components[props.component]?.[props.channel] as Quantity | undefined,
    }))
    .filter((point): point is { time: number; quantity: Quantity } => point.quantity !== undefined),
)

/** How to read this quantity, decided once from everything on screen. */
const scale = computed(() =>
  scaleFor(
    props.unit,
    points.value.flatMap((point) => [point.quantity.p10, point.quantity.p90]),
  ),
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

  // A proportion is drawn against the whole of its range. Adding breathing room
  // above it would label the top of the axis 108%, which is not a share of
  // anything, and it costs nothing to show the ceiling the value is approaching.
  if (scale.value.factor === 100) return { low: 0, high: 1 }

  if (high === low) return { low: low - 0.5, high: high + 0.5 }
  const margin = (high - low) * 0.08
  return {
    // Nothing solved here is negative — rates, times, occupancies and shares are
    // all non-negative — so the margin is not allowed to push the axis below
    // zero and label a duration as minus twenty milliseconds.
    low: low >= 0 ? Math.max(0, low - margin) : low - margin,
    high: high + margin,
  }
})

const plot = {
  width: WIDTH - PADDING.left - PADDING.right,
  height: props.height - PADDING.top - PADDING.bottom,
}

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
  const lower = points.value.map((point, index) => `${x(index)},${y(point.quantity.p10)}`).reverse()
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

/**
 * Where the readout sits, in percentages of the frame.
 *
 * It follows the hovered point rather than living in a fixed footer, so the eye
 * does not have to travel between the value and the place it came from. Near the
 * right-hand edge it flips to the other side of the cursor so it stays inside
 * the chart.
 */
const anchor = computed(() => {
  if (hovered.value === null) return null
  const at = (x(hovered.value) / WIDTH) * 100
  return { left: `${at}%`, flipped: at > 62 }
})

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
  const w = 168
  const h = 30
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
      <span class="unit">{{ scale.suffix || unit || '1' }}</span>
    </figcaption>

    <div class="frame">
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
          {{ showScaled(tick.value, scale) }}
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
          <circle class="dot" :cx="x(hovered)" :cy="y(active.quantity.p50)" r="3.5" />
        </template>
      </svg>

      <!--
        The step's own distribution, beside the point it belongs to. A line of
        medians hides whether a value is one outcome or two, which for a design
        near a fold is the thing being looked for, so stopping on a point has to
        show the shape behind it rather than a number in a footer somewhere else.
      -->
      <div
        v-if="active && anchor"
        class="readout"
        :class="{ flipped: anchor.flipped }"
        :style="{ left: anchor.left }"
        data-test="step-readout"
      >
        <div class="head">
          <span class="time">t = {{ formatSiNumber(active.time) }}s</span>
          <el-tag v-if="sketch && sketch.modes > 1" type="warning" size="small" effect="light">
            {{ sketch.modes }} states
          </el-tag>
        </div>
        <div class="value">{{ showWithUnit(active.quantity.p50, scale) }}</div>
        <svg v-if="sketch" :viewBox="`0 0 ${sketch.width} ${sketch.height}`" class="sketch">
          <path :d="sketch.path" />
        </svg>
        <p v-else class="certain">certain</p>
        <dl class="quantiles">
          <div>
            <dt>p10</dt>
            <dd>{{ showScaled(active.quantity.p10, scale) }}</dd>
          </div>
          <div>
            <dt>mean</dt>
            <dd>{{ showScaled(active.quantity.mean, scale) }}</dd>
          </div>
          <div>
            <dt>p90</dt>
            <dd>{{ showScaled(active.quantity.p90, scale) }}</dd>
          </div>
        </dl>
      </div>
    </div>
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
.frame { position: relative; }
.plot { width: 100%; display: block; cursor: crosshair; }
.gridline { stroke: var(--line); stroke-width: 1; }
.tick { font-family: var(--mono); font-size: 9px; fill: var(--muted); }
.band { fill: var(--green); fill-opacity: 0.16; }
.median { fill: none; stroke: var(--green); stroke-width: 1.75; stroke-linejoin: round; }
.cursor { stroke: var(--ink); stroke-width: 1; stroke-dasharray: 2 2; }
.dot { fill: var(--green); stroke: var(--surface-strong); stroke-width: 1.5; }

.readout {
  position: absolute;
  top: 2px;
  transform: translateX(10px);
  width: 182px;
  padding: 6px var(--space-2);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface-strong);
  box-shadow: 0 6px 20px rgb(28 35 31 / 14%);
  pointer-events: none;
  z-index: 2;
}
.readout.flipped { transform: translateX(calc(-100% - 10px)); }
.head { display: flex; align-items: center; justify-content: space-between; gap: var(--space-2); }
.time { font-family: var(--mono); font-size: var(--text-2xs); color: var(--muted); }
.value { font-family: var(--mono); font-size: var(--text-md); line-height: 1.2; margin: 1px 0 2px; }
.sketch { width: 100%; height: 30px; display: block; }
.sketch path { fill: var(--green); fill-opacity: 0.45; stroke: var(--green); stroke-width: 1.25; }
.certain { margin: var(--space-1) 0; font-size: var(--text-2xs); color: var(--muted); font-style: italic; }
.quantiles { display: flex; justify-content: space-between; margin: 2px 0 0; gap: var(--space-2); }
.quantiles div { display: flex; flex-direction: column; align-items: center; }
.quantiles dt { font-size: 9px; color: var(--muted); text-transform: uppercase; letter-spacing: 0.04em; }
.quantiles dd { margin: 0; font-family: var(--mono); font-size: var(--text-2xs); }
</style>
