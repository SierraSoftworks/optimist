<script setup lang="ts">
import { computed, ref } from 'vue'

import type { Frame, Quantity } from '../api/types'
import { densityProfile, kernelDensity } from '../domain/density'
import { formatSiNumber } from '../domain/humanNumber'
import { scaleFor, showScaled, showWithUnit } from '../domain/units'

const props = withDefaults(
  defineProps<{
    series: Frame[]
    component: string
    channel: string
    label?: string
    unit?: string
    height?: number
    /**
     * The same quantity in the design as it stands, where a variant is on show.
     *
     * Given as a whole series rather than a single figure because a variant can
     * change when something happens as well as whether it does, and a reader
     * comparing two end states would miss a proposal that merely postpones the
     * collapse it was meant to prevent.
     */
    baseline?: Frame[]
    /** What the baseline is called, for the legend and the readout. */
    baselineLabel?: string
  }>(),
  { label: '', unit: '', height: 120, baseline: undefined, baselineLabel: 'as designed' },
)

const WIDTH = 640
const PADDING = { left: 52, right: 10, top: 10, bottom: 20 }

/** Width of the step readout, which has to be known to keep it inside the frame. */
const READOUT = 182

/** The quantity at each step, dropping steps where this channel is absent. */
const points = computed(() => read(props.series))

/** The same quantity in the design this variant would replace. */
const reference = computed(() => (props.baseline ? read(props.baseline) : []))

function read(frames: Frame[]) {
  return frames
    .map((frame) => ({
      time: frame.time,
      quantity: frame.components[props.component]?.[props.channel] as Quantity | undefined,
    }))
    .filter((point): point is { time: number; quantity: Quantity } => point.quantity !== undefined)
}

/** Whether there is a baseline to compare against, and it lines up with this one. */
const comparing = computed(
  () => reference.value.length > 0 && reference.value.length === points.value.length,
)

/** How to read this quantity, decided once from everything on screen. */
const scale = computed(() =>
  scaleFor(props.unit, [
    ...points.value.flatMap((point) => [point.quantity.p10, point.quantity.p90]),
    ...reference.value.map((point) => point.quantity.mean),
  ]),
)

/**
 * Vertical range covering the whole band, not just the means.
 *
 * Scaling to the mean would clip the spread that the shading exists to show, and
 * a reader would see a confident line where the model is anything but.
 */
const bounds = computed(() => {
  const values = [
    ...points.value.flatMap((point) => [point.quantity.p10, point.quantity.mean, point.quantity.p90]),
    // The baseline is drawn on the same axis, so it has to fit on it. A variant
    // that halved a latency would otherwise put its reference line off the top
    // of the chart, which is where the whole point of the comparison lives.
    ...reference.value.map((point) => point.quantity.mean),
  ]
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

/**
 * The line, which tracks the mean.
 *
 * The median was the obvious choice and was wrong for the designs this tool
 * exists to draw. Where a design divides between a healthy branch and a
 * collapsed one, the median sits on whichever branch happens to hold more than
 * half the draws and jumps to the other the moment that majority tips. A design
 * whose success rate averaged a third was therefore drawn as a flat line along
 * zero, and a variant that barely moved the distribution was drawn as a
 * transformation because it had moved the majority by a few per cent. The mean
 * is continuous in the mixture and does not do that. It is also, on its own,
 * a figure no request ever sees — which is what the shading behind it is for.
 */
const average = computed(() =>
  points.value.map((point, index) => `${x(index)},${y(point.quantity.mean)}`).join(' '),
)

/** The p10–p90 band, drawn as one closed path down the upper edge and back. */
const band = computed(() => {
  if (points.value.length < 2) return ''
  const upper = points.value.map((point, index) => `${x(index)},${y(point.quantity.p90)}`)
  const lower = points.value.map((point, index) => `${x(index)},${y(point.quantity.p10)}`).reverse()
  return `M${upper.join(' L')} L${lower.join(' L')} Z`
})

/** Shades between which a step's density is drawn, darkest for the likeliest. */
const SHADES = 5

/** Cells each step's column is divided into vertically. */
const ROWS = 44

/**
 * The distribution at every step, shaded by how much of it sits where.
 *
 * A band between two percentiles says where the middle four fifths of the draws
 * are and nothing about how they are arranged inside it. For most designs that
 * is a fair summary. For the ones worth simulating it is not: a design at a fold
 * puts a third of its draws at one value and two thirds at another, and drawn as
 * a band it looks identical to a design that is merely uncertain, with the
 * heaviest ink laid exactly where no draw landed.
 *
 * So each step is drawn as its own density: dark where the draws are, empty
 * where they are not, and visibly split where the design is. Every column is
 * scaled to its own tallest point, because the question a reader is asking of
 * each one is the shape of that moment rather than how it compares with the
 * busiest moment in the run.
 *
 * The cells are collected into one path per shade rather than drawn
 * individually. A horizon of a few hundred steps is ordinary, and a rectangle
 * per cell would put tens of thousands of nodes in the document for a figure a
 * few hundred pixels wide.
 */
const shading = computed(() => {
  const count = points.value.length
  if (count === 0) return []
  const { low, high } = bounds.value
  if (!(high > low)) return []
  const cell = plot.height / ROWS
  const shades: string[] = Array.from({ length: SHADES }, () => '')
  let drew = false

  for (let index = 0; index < count; index += 1) {
    const profile = densityProfile(points.value[index].quantity.draws, low, high, ROWS)
    if (!profile) continue
    drew = true
    // Columns meet at the midpoints between steps, so the shading tiles without
    // gaps while each step still owns the space nearest to it. No value exists
    // between two steps, and this interpolates none: it fills a step's own
    // neighbourhood with that step's answer.
    const left = index === 0 ? x(0) : (x(index - 1) + x(index)) / 2
    const right = index === count - 1 ? x(count - 1) : (x(index) + x(index + 1)) / 2
    const width = right - left
    if (!(width > 0)) continue

    // Runs of equal shade are merged into one rectangle on the way down, which
    // is most of the column for the flat-topped densities a saturated design
    // produces.
    let run = shadeOf(profile[0])
    let from = 0
    for (let row = 1; row <= ROWS; row += 1) {
      const shade = row < ROWS ? shadeOf(profile[row]) : -1
      if (shade === run) continue
      if (run > 0) {
        // Row zero is the bottom of the axis, so it is drawn from the bottom up.
        const top = PADDING.top + plot.height - row * cell
        shades[run - 1] += `M${left},${top}h${width}v${(row - from) * cell}h${-width}z`
      }
      run = shade
      from = row
    }
  }
  if (!drew) return []
  return shades
    .map((path, index) => ({ path, opacity: (index + 1) / SHADES }))
    .filter((shade) => shade.path.length > 0)
})

/**
 * Which shade a density belongs in.
 *
 * Compressed by a square root so that a branch holding a tenth of the draws —
 * which peaks at about a tenth of the majority's height — is drawn at a third of
 * the ink rather than a tenth of it. A minority branch is the thing this chart
 * exists to show, and linear ink would leave it invisible.
 */
function shadeOf(density: number): number {
  const shade = Math.ceil(Math.sqrt(Math.max(density, 0)) * SHADES)
  return Math.min(shade, SHADES)
}

/** The baseline's means, drawn dashed so it reads as a reference not a result. */
const referenceLine = computed(() =>
  comparing.value
    ? reference.value.map((point, index) => `${x(index)},${y(point.quantity.mean)}`).join(' ')
    : '',
)

/**
 * The ground between the two lines.
 *
 * Shaded rather than left as two lines because the quantity being judged is the
 * gap, and a reader asked to measure the distance between two lines by eye will
 * get the sign right and the size wrong. The fill is one neutral tint in both
 * directions: whether a rise is an improvement depends on what is being charted,
 * and colouring it green would decide that on the reader's behalf — wrongly,
 * every time the quantity is a latency.
 */
const difference = computed(() => {
  if (!comparing.value || points.value.length < 2) return ''
  const variant = points.value.map((point, index) => `${x(index)},${y(point.quantity.mean)}`)
  const settled = reference.value
    .map((point, index) => `${x(index)},${y(point.quantity.mean)}`)
    .reverse()
  return `M${variant.join(' L')} L${settled.join(' L')} Z`
})

const hovered = ref<number | null>(null)
const active = computed(() =>
  hovered.value === null ? null : (points.value[hovered.value] ?? null),
)

/** The baseline at the hovered step, where there is one to compare against. */
const activeReference = computed(() =>
  hovered.value === null || !comparing.value ? null : (reference.value[hovered.value] ?? null),
)

/**
 * How far the variant has moved the quantity at the hovered step.
 *
 * Reported as a share of the baseline as well as in the quantity's own units,
 * because "eighty milliseconds slower" and "twice as slow" are different claims
 * and which one matters depends on where the reader started.
 */
const shift = computed(() => {
  const settled = activeReference.value?.quantity.mean
  const now = active.value?.quantity.mean
  if (settled === undefined || now === undefined) return null
  const absolute = now - settled
  const relative = settled === 0 ? null : absolute / Math.abs(settled)
  return {
    direction: absolute > 0 ? 'up' : absolute < 0 ? 'down' : 'level',
    size: showScaled(Math.abs(absolute), scale.value),
    // A change of a few per cent rounds to nothing at whole numbers, and "0%"
    // beside a visible gap reads as a bug rather than as a small difference.
    share:
      relative === null
        ? null
        : `${(Math.abs(relative) * 100).toFixed(Math.abs(relative) < 0.1 ? 1 : 0)}%`,
  }
})

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
  const flipped = at > 62
  // Clamped in CSS rather than by measuring, because the chart is fluid and the
  // readout is not: near either edge of a narrow column it would otherwise hang
  // outside the figure, over the sidebar or off the page.
  const wanted = flipped ? `calc(${at}% - ${READOUT + 10}px)` : `calc(${at}% + 10px)`
  return { left: `clamp(0px, ${wanted}, calc(100% - ${READOUT}px))` }
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
      <span class="channel">{{ label || channel }}</span>
      <span class="component">{{ component }}</span>
      <span v-if="comparing" class="legend" data-test="baseline-legend">
        <span class="key variant"></span>this variant
        <span class="key settled"></span>{{ baselineLabel }}
      </span>
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

        <path v-if="difference" class="difference" :d="difference" />
        <path v-if="!shading.length && band" class="band" :d="band" />
        <path
          v-for="shade in shading"
          :key="shade.opacity"
          class="density"
          :d="shade.path"
          :fill-opacity="shade.opacity * 0.34"
        />
        <polyline v-if="referenceLine" class="reference" :points="referenceLine" />
        <polyline v-if="average" class="average" :points="average" />

        <template v-if="hovered !== null && active">
          <line
            class="cursor"
            :x1="x(hovered)"
            :x2="x(hovered)"
            :y1="PADDING.top"
            :y2="PADDING.top + plot.height"
          />
          <circle
            v-if="activeReference"
            class="dot settled"
            :cx="x(hovered)"
            :cy="y(activeReference.quantity.mean)"
            r="3"
          />
          <circle class="dot" :cx="x(hovered)" :cy="y(active.quantity.mean)" r="3.5" />
        </template>
      </svg>

      <!--
        The step's own distribution, beside the point it belongs to. The chart
        behind it shades where the draws are, which answers how many states there
        are but not what they are worth; stopping on a point puts numbers on it.
      -->
      <div
        v-if="active && anchor"
        class="readout"
        :style="{ left: anchor.left }"
        data-test="step-readout"
      >
        <div class="head">
          <span class="time">t = {{ formatSiNumber(active.time) }}s</span>
          <el-tag v-if="sketch && sketch.modes > 1" type="warning" size="small" effect="light">
            {{ sketch.modes }} states
          </el-tag>
        </div>
        <div class="value">{{ showWithUnit(active.quantity.mean, scale) }}</div>
        <p v-if="shift" class="shift" data-test="baseline-shift">
          <span class="delta" :class="shift.direction">
            <span class="arrow">{{ shift.direction === 'up' ? '▲' : shift.direction === 'down' ? '▼' : '—' }}</span>
            <span>{{ shift.size }}</span>
            <span v-if="shift.share" class="share">{{ shift.share }}</span>
          </span>
          <span class="against">vs {{ baselineLabel }}</span>
        </p>
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
            <dt>p50</dt>
            <dd>{{ showScaled(active.quantity.p50, scale) }}</dd>
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
figcaption {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  margin-bottom: var(--space-1);
  min-width: 0;
}
.channel { font-family: var(--mono); font-size: var(--text-sm); font-weight: 650; white-space: nowrap; }
.component {
  font-size: var(--text-2xs);
  color: var(--muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}
.unit { font-family: var(--mono); font-size: var(--text-2xs); color: var(--muted); flex: 0 0 auto; }
.legend {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 10px;
  color: var(--muted);
  white-space: nowrap;
  flex: 0 0 auto;
}
.legend .key { width: 12px; height: 0; border-top: 2px solid var(--green); display: inline-block; }
.legend .key.settled { border-top-style: dashed; border-top-color: var(--muted); }
.legend .key + .key { margin-left: var(--space-2); }
.frame { position: relative; }
.plot { width: 100%; display: block; cursor: crosshair; }
.gridline { stroke: var(--line); stroke-width: 1; }
.tick { font-family: var(--mono); font-size: 9px; fill: var(--muted); }
.band { fill: var(--green); fill-opacity: 0.16; }
/*
 * One path per shade, laid over each other. Opacity is set per path from the
 * density rather than in the stylesheet, so the tint carries the figure.
 */
.density { fill: var(--green); stroke: none; }
/*
 * The gap between the two, filled in one tint whichever way it goes. Which
 * direction counts as an improvement depends on the quantity, and a chart that
 * decided that for the reader would be wrong every time it drew a latency.
 */
.difference { fill: var(--ink); fill-opacity: 0.1; }
.reference { fill: none; stroke: var(--muted); stroke-width: 1.5; stroke-dasharray: 4 3; stroke-linejoin: round; }
.average { fill: none; stroke: var(--green); stroke-width: 1.75; stroke-linejoin: round; }
.cursor { stroke: var(--ink); stroke-width: 1; stroke-dasharray: 2 2; }
.dot { fill: var(--green); stroke: var(--surface-strong); stroke-width: 1.5; }
.dot.settled { fill: var(--muted); }

.readout {
  position: absolute;
  top: 2px;
  width: 182px;
  padding: 6px var(--space-2);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface-strong);
  box-shadow: 0 6px 20px rgb(28 35 31 / 14%);
  pointer-events: none;
  z-index: 2;
}
.head { display: flex; align-items: center; justify-content: space-between; gap: var(--space-2); }
.time { font-family: var(--mono); font-size: var(--text-2xs); color: var(--muted); }
.value { font-family: var(--mono); font-size: var(--text-md); line-height: 1.2; margin: 1px 0 2px; }
.shift { margin: 0 0 var(--space-1); display: flex; align-items: baseline; gap: var(--space-1); flex-wrap: wrap; }
.delta { display: inline-flex; align-items: baseline; gap: 3px; font-family: var(--mono); font-size: var(--text-2xs); }
.delta .arrow { font-size: 8px; }
.delta .share { color: var(--muted); }
.delta.level { color: var(--muted); }
.against { font-size: 10px; color: var(--muted); }
.sketch { width: 100%; height: 30px; display: block; }
.sketch path { fill: var(--green); fill-opacity: 0.45; stroke: var(--green); stroke-width: 1.25; }
.certain { margin: var(--space-1) 0; font-size: var(--text-2xs); color: var(--muted); font-style: italic; }
.quantiles { display: flex; justify-content: space-between; margin: 2px 0 0; gap: var(--space-2); }
.quantiles div { display: flex; flex-direction: column; align-items: center; }
.quantiles dt { font-size: 9px; color: var(--muted); text-transform: uppercase; letter-spacing: 0.04em; }
.quantiles dd { margin: 0; font-family: var(--mono); font-size: var(--text-2xs); }
</style>
