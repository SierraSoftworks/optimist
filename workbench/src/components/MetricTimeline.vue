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
const SHADES = 8

/** Cells each step's column is divided into vertically. */
const ROWS = 56

/**
 * Density below which a row is left out of every band, as a fraction of the
 * column's peak.
 *
 * Every kernel estimate has ripples in its tails, and without a floor they are
 * drawn as a faint wash across the whole axis — which is exactly the false
 * confidence a band between two percentiles gives, arrived at another way. The
 * floor is far below a real minority branch, which holds a tenth of the draws
 * and so peaks at about a tenth of the majority's height.
 */
const MIN_INK = 0.02

/**
 * Ink laid down by one band.
 *
 * The bands nest, so a point at the peak is covered by all {@link SHADES} of
 * them and ends up at `1 - (1 - LAYER) ** SHADES`, a little under a half. One
 * band alone is barely there, which is what the edge of a distribution should
 * look like.
 */
const LAYER = 0.075

/** A contiguous stretch of rows in one step's column, as `[from, to)`. */
interface Run {
  from: number
  to: number
}

/** One step's run at one shade, and which step it belongs to. */
interface Link {
  column: number
  run: Run
}

/**
 * The distribution over time, drawn as bands of equal density.
 *
 * A band between two percentiles says where the middle four fifths of the draws
 * are and nothing about how they are arranged inside it. For most designs that
 * is a fair summary. For the ones worth simulating it is not: a design at a fold
 * puts a third of its draws at one value and two thirds at another, and drawn as
 * a percentile band it looks identical to a design that is merely uncertain,
 * with the heaviest ink laid exactly where no draw landed.
 *
 * So the shading follows the density instead. Each shade covers the region where
 * the density is at least that high, and the regions nest, so where they lie on
 * top of one another the ink accumulates into the peak. Crucially a region is
 * allowed to be in two pieces: where a design divides, so does the shading, and
 * the gap between the branches is left white.
 *
 * Each piece is carried from step to step as one continuous ribbon rather than
 * drawn as a column per step. A branch that persists then reads as a shape with
 * a lower and an upper edge the eye can follow, which is what makes the mean
 * line legible as something running between them rather than through a haze, and
 * one path per band leaves no seams for the renderer to show at the joins. A
 * branch that opens or closes is joined to the nearest branch on the step beside
 * it, because its draws were somewhere on that step too.
 *
 * Every column is scaled to its own tallest point, because what a reader asks of
 * each one is the shape of that moment rather than how it compares with the
 * busiest moment in the run.
 */
const shading = computed(() => {
  const count = points.value.length
  if (count === 0) return []
  const { low, high } = bounds.value
  if (!(high > low)) return []
  const profiles = points.value.map((point) =>
    densityProfile(point.quantity.draws, low, high, ROWS),
  )
  if (!profiles.some(Boolean)) return []

  return Array.from({ length: SHADES }, (_, index) => {
    const shade = index + 1
    return {
      shade,
      path: ribbons(profiles.map((profile) => (profile ? runsAt(profile, shade) : []))),
    }
  }).filter((band) => band.path.length > 0)
})

/** The stretches of a column whose density reaches `shade`. */
function runsAt(profile: number[], shade: number): Run[] {
  const runs: Run[] = []
  let from = -1
  for (let row = 0; row <= profile.length; row += 1) {
    const inside = row < profile.length && shadeOf(profile[row]) >= shade
    if (inside && from < 0) from = row
    if (!inside && from >= 0) {
      runs.push({ from, to: row })
      from = -1
    }
  }
  return runs
}

/**
 * Threads one shade's runs into ribbons.
 *
 * A run continues into the run beside it that it shares the most rows with,
 * which is the only sense in which two steps can be said to be showing the same
 * branch: they are not the same draws, and nothing in the model gives a branch
 * an identity that survives a step. Where two branches merge, the first to claim
 * the run beside it carries on and the other ends, which is what a merge looks
 * like.
 */
function ribbons(columns: Run[][]): string {
  const paths: string[] = []
  const threaded = columns.map((runs) => runs.map(() => false))

  for (let column = 0; column < columns.length; column += 1) {
    for (let index = 0; index < columns[column].length; index += 1) {
      if (threaded[column][index]) continue
      const chain: Link[] = []
      let at = column
      let which = index
      while (at < columns.length && which >= 0 && !threaded[at][which]) {
        threaded[at][which] = true
        chain.push({ column: at, run: columns[at][which] })
        which = at + 1 < columns.length ? overlapping(columns[at][which], columns[at + 1]) : -1
        at += 1
      }
      paths.push(outline(chain, columns))
    }
  }
  return paths.join('')
}

/** Which of `candidates` shares the most rows with `run`, or -1 if none does. */
function overlapping(run: Run, candidates: Run[]): number {
  let best = -1
  let shared = 0
  candidates.forEach((candidate, index) => {
    const rows = Math.min(run.to, candidate.to) - Math.max(run.from, candidate.from)
    if (rows > shared) {
      shared = rows
      best = index
    }
  })
  return best
}

/**
 * Where a ribbon that opens or closes at `run` meets the step beside it.
 *
 * The nearest point of the nearest branch there, so the ribbon grows out of that
 * branch rather than out of nowhere. A collapse that takes a design from certain
 * success to a split between success and failure is one continuous shape opening
 * downward from the value it left, which is what happened; tapering it to a
 * point at its own mid-height instead drew the failed branch as having arrived
 * from a value the design was never at.
 *
 * Null where that step has no shading at this depth at all, which is the only
 * case in which a branch really does begin from nothing.
 */
function joins(run: Run, neighbours: Run[]): number | null {
  const centre = (run.from + run.to) / 2
  const apart = (candidate: Run) =>
    Math.max(candidate.from - centre, centre - candidate.to, 0)
  const nearest = neighbours.reduce<Run | null>(
    (best, candidate) => (best === null || apart(candidate) < apart(best) ? candidate : best),
    null,
  )
  return nearest === null ? null : Math.min(Math.max(centre, nearest.from), nearest.to)
}

/** One ribbon: out along its upper edge and back along its lower one. */
function outline(chain: Link[], columns: Run[][]): string {
  const cell = plot.height / ROWS
  // Row zero is the bottom of the axis, so rows count upward from the baseline.
  const at = (row: number) => PADDING.top + plot.height - row * cell
  const middle = (run: Run) => at((run.from + run.to) / 2)

  const first = chain[0]
  const last = chain[chain.length - 1]
  const corners: string[] = []
  if (first.column > 0) {
    const joined = joins(first.run, columns[first.column - 1])
    corners.push(
      joined === null
        ? `${(x(first.column - 1) + x(first.column)) / 2},${middle(first.run)}`
        : `${x(first.column - 1)},${at(joined)}`,
    )
  }
  for (const link of chain) corners.push(`${x(link.column)},${at(link.run.to)}`)
  if (last.column < columns.length - 1) {
    const joined = joins(last.run, columns[last.column + 1])
    corners.push(
      joined === null
        ? `${(x(last.column) + x(last.column + 1)) / 2},${middle(last.run)}`
        : `${x(last.column + 1)},${at(joined)}`,
    )
  }
  for (const link of [...chain].reverse()) corners.push(`${x(link.column)},${at(link.run.from)}`)
  return `M${corners.join(' L')}Z`
}

/**
 * Which band a density reaches.
 *
 * Linear, because the layering does the lifting: a point covered by `k` bands
 * ends up at `1 - (1 - LAYER) ** k`, which already gives the first few bands
 * more ink than their share and so keeps a minority branch visible. Compressing
 * the input as well left the valley between two branches half as dark as the
 * branches themselves, which is the shape being looked for drawn as a smear.
 */
function shadeOf(density: number): number {
  if (!(density > MIN_INK)) return 0
  return Math.min(Math.ceil(density * SHADES), SHADES)
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
          v-for="band in shading"
          :key="band.shade"
          class="density"
          :d="band.path"
          :fill-opacity="LAYER"
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
