<script setup lang="ts">
import { computed } from 'vue'
import type { ObjectiveTrajectoryPoint } from '../api/types'
import { impactTone } from '../domain/optimizationImpact'
import { outcomeScale, positionOn } from '../domain/outcomeScale'

const props = defineProps<{
  points: ObjectiveTrajectoryPoint[]
  reference: Array<number | null>
  label: string
  unit: string | null
  direction: 'maximize' | 'minimize'
  /** Whether `reference` is a projected run rather than the resting level. */
  projectedReference: boolean
}>()

const width = 320
const height = 132
const inset = { top: 12, right: 12, bottom: 24, left: 46 }
const usableWidth = width - inset.left - inset.right
const usableHeight = height - inset.top - inset.bottom

const states = computed(() => props.points.map((point) => point.state.mean))
const spread = computed(() => props.points.map((point) => {
  const variance = point.state.variance
  return variance === null ? 0 : Math.sqrt(Math.max(0, variance))
}))
const upper = computed(() => states.value.map(
  (state, index) => state === null ? null : state + spread.value[index]!,
))
const lower = computed(() => states.value.map(
  (state, index) => state === null ? null : Math.max(state - spread.value[index]!, 0),
))
const scale = computed(() => outcomeScale(
  [...states.value, ...props.reference],
  [...upper.value, ...lower.value],
))

/**
 * Tone follows the gap between the two runs, not movement over time.
 *
 * An outcome may climb under both runs; what the reader is deciding is whether
 * this intervention leaves them better off than not running it, so the colour
 * answers that question and nothing else.
 */
const tone = computed(() => {
  const candidate = states.value.at(-1)
  const without = props.reference.at(-1)
  if (candidate === null || candidate === undefined || without === null || without === undefined) {
    return 'neutral'
  }
  return impactTone(candidate - without, props.direction)
})

const candidateLine = computed(() => line(states.value))
const referenceLine = computed(() => line(props.reference))
const upperLine = computed(() => line(upper.value))
const lowerLine = computed(() => line(lower.value))
const deviation = computed(() => area(states.value, props.reference))
const axis = computed(() => `${format(scale.value.upper)} to ${format(scale.value.lower)} ${props.unit ?? ''}`)

function x(index: number) {
  const count = props.points.length
  return inset.left + (count <= 1 ? 0 : (index / (count - 1)) * usableWidth)
}

function y(value: number) {
  return inset.top + (1 - positionOn(scale.value, value)) * usableHeight
}

function line(values: Array<number | null>) {
  return values
    .flatMap((value, index) => value === null ? [] : [`${index ? 'L' : 'M'} ${x(index)} ${y(value)}`])
    .join(' ')
}

/** Closes the region between two series, skipping periods either one is missing. */
function area(top: Array<number | null>, bottom: Array<number | null>) {
  const paired = top.flatMap((value, index) => {
    const other = bottom[index]
    return value === null || other === null || other === undefined ? [] : [{ index, value, other }]
  })
  if (!paired.length) return ''
  const forward = paired.map((entry, position) => `${position ? 'L' : 'M'} ${x(entry.index)} ${y(entry.value)}`)
  const back = [...paired].reverse().map((entry) => `L ${x(entry.index)} ${y(entry.other)}`)
  return `${forward.join(' ')} ${back.join(' ')} Z`
}

/**
 * Renders a value short enough for the axis gutter.
 *
 * A logarithmic axis over a saturating quantity puts a very small number at the
 * bottom, and its decimal form is wide enough to run off the chart. Exponent
 * form keeps every label the same handful of characters.
 */
function format(value: number) {
  const magnitude = Math.abs(value)
  if (magnitude >= 1e6) return `${Number((value / 1e6).toPrecision(3))}M`
  if (magnitude >= 1e4) return `${Number((value / 1e3).toPrecision(3))}K`
  if (magnitude > 0 && magnitude < 0.01) return value.toExponential(1)
  return Number(value.toPrecision(3)).toString()
}

function describe(index: number) {
  const state = states.value[index]
  const without = props.reference[index]
  const suffix = props.unit ? ` ${props.unit}` : ''
  if (state === null || state === undefined) return 'unavailable'
  const withoutText = without === null || without === undefined
    ? ''
    : `, without it ${format(without)}${suffix}`
  return `${format(state)}${suffix}${withoutText}`
}
</script>

<template>
  <figure class="trajectory" :data-impact="tone" :aria-label="`${label} over time, with and without this intervention`">
    <figcaption>
      <strong>{{ label }}</strong>
      <span>
        {{ direction }} · {{ scale.logarithmic ? 'log' : 'linear' }} {{ unit ?? 'scale' }} · ±1 SD ·
        <template v-if="projectedReference">dashed line is the prerequisites alone</template>
        <template v-else>dashed line is the resting level</template>
      </span>
    </figcaption>
    <svg :viewBox="`0 0 ${width} ${height}`" role="img">
      <path v-if="deviation" :d="deviation" class="deviation" />
      <path v-if="upperLine" :d="upperLine" class="spread-line" />
      <path v-if="lowerLine" :d="lowerLine" class="spread-line" />
      <path v-if="referenceLine" :d="referenceLine" class="reference-line" />
      <path v-if="candidateLine" :d="candidateLine" class="trajectory-line" />
      <circle
        v-for="(point, index) in points"
        :key="point.period"
        :cx="x(index)"
        :cy="y(states[index] ?? scale.lower)"
        r="2.5"
      >
        <title>Period {{ point.period }}: {{ describe(index) }}</title>
      </circle>
      <text :x="inset.left" :y="height - 6">0</text>
      <text :x="width - inset.right" :y="height - 6" text-anchor="end">{{ points.at(-1)?.period ?? 0 }} periods</text>
      <text x="2" :y="inset.top + 4">{{ format(scale.upper) }}</text>
      <text x="2" :y="height - inset.bottom">{{ format(scale.lower) }}</text>
    </svg>
    <ol class="sr-only">
      <li>Axis {{ axis }}</li>
      <li v-for="(point, index) in points" :key="point.period">Period {{ point.period }}: {{ describe(index) }}</li>
    </ol>
  </figure>
</template>

<style scoped>
.trajectory { margin: 0; padding: var(--space-3) var(--space-4); border-top: 1px solid var(--line); background: #fbfcfa; }
.trajectory figcaption { display: flex; flex-wrap: wrap; align-items: baseline; justify-content: space-between; gap: 8px; }
.trajectory figcaption strong { font-size: var(--text-md); }
.trajectory figcaption span { color: var(--muted); font-size: var(--text-xs); }
svg { display: block; width: 100%; height: auto; margin-top: var(--space-2); overflow: visible; }
text { fill: var(--muted); font-size: var(--text-2xs); font-family: var(--mono); }
.reference-line { fill: none; stroke: var(--muted); stroke-width: 1.25; stroke-dasharray: 4 3; opacity: 0.85; }
.trajectory-line { fill: none; stroke-width: 2; }
.spread-line { fill: none; stroke-width: 0.75; stroke-dasharray: 1 2; opacity: 0.55; }
.deviation { stroke: none; opacity: 0.22; }
circle { fill: currentColor; }
[data-impact='positive'] { color: #277445; }
[data-impact='negative'] { color: #a34335; }
[data-impact='neutral'] { color: var(--muted); }
[data-impact] .trajectory-line,
[data-impact] .spread-line { stroke: currentColor; }
[data-impact] .deviation { fill: currentColor; }
</style>
