<script setup lang="ts">
import { computed } from 'vue'
import type { ObjectiveTrajectoryPoint } from '../api/types'
import { impactTone, relativeImprovement } from '../domain/optimizationImpact'

const props = defineProps<{
  points: ObjectiveTrajectoryPoint[]
  label: string
  direction: 'maximize' | 'minimize'
  baseline: number | null
}>()

const width = 320
const height = 122
const inset = { top: 12, right: 12, bottom: 24, left: 34 }
const usableWidth = width - inset.left - inset.right
const usableHeight = height - inset.top - inset.bottom
const values = computed(() => props.points.flatMap((point) => {
  const mean = relativeImprovement(point.improvement.mean, props.baseline)
  if (mean === null) return []
  const spread = normalizedSpread(point)
  return [mean - spread, mean + spread, 0]
}))
const domain = computed(() => {
  const minimum = Math.min(...values.value, 0)
  const maximum = Math.max(...values.value, 0)
  const padding = Math.max((maximum - minimum) * 0.12, 0.01)
  return [minimum - padding, maximum + padding] as const
})
const line = computed(() => path(props.points.map((point) => relativeImprovement(point.improvement.mean, props.baseline))))
const band = computed(() => {
  const upper = props.points.map((point) => bound(point, 1))
  const lower = props.points.map((point) => bound(point, -1)).reverse()
  return `${pathPoints(upper)} ${pathPoints(lower, true)}`
})
const zeroY = computed(() => y(0))
const finalImpact = computed(() => {
  const baseline = props.points[0]?.state.mean
  const finalState = props.points.at(-1)?.state.mean
  return impactTone(
    baseline === null || baseline === undefined || finalState === null || finalState === undefined
      ? null
      : finalState - baseline,
    props.direction,
  )
})

function x(index: number) {
  return inset.left + (props.points.length <= 1 ? 0 : index / (props.points.length - 1) * usableWidth)
}

function y(value: number) {
  const [minimum, maximum] = domain.value
  return inset.top + (maximum - value) / (maximum - minimum) * usableHeight
}

function bound(point: ObjectiveTrajectoryPoint, direction: number) {
  const mean = relativeImprovement(point.improvement.mean, props.baseline) ?? 0
  const spread = normalizedSpread(point)
  return mean + direction * spread
}

function normalizedSpread(point: ObjectiveTrajectoryPoint) {
  if (point.improvement.variance === null) return 0
  return relativeImprovement(Math.sqrt(Math.max(0, point.improvement.variance)), props.baseline) ?? 0
}

function path(points: Array<number | null>) {
  return points.flatMap((value, index) => value === null ? [] : [`${index ? 'L' : 'M'} ${x(index)} ${y(value)}`]).join(' ')
}

function pathPoints(points: number[], continuePath = false) {
  return points.map((value, index) => `${continuePath || index ? 'L' : 'M'} ${x(continuePath ? points.length - index - 1 : index)} ${y(value)}`).join(' ')
}

function format(value: number) {
  return `${Number((value * 100).toPrecision(3))}%`
}
</script>

<template>
  <figure class="trajectory" :data-impact="finalImpact" :aria-label="`${label} relative improvement over time`">
    <figcaption><strong>{{ label }}</strong><span>Improvement vs baseline · {{ direction }} · ±1 SD</span></figcaption>
    <svg :viewBox="`0 0 ${width} ${height}`" role="img">
      <line :x1="inset.left" :x2="width - inset.right" :y1="zeroY" :y2="zeroY" class="zero-line" />
      <path v-if="points.length" :d="`${band} Z`" class="uncertainty-band" />
      <path v-if="points.length" :d="line" class="trajectory-line" />
      <circle v-for="(point, index) in points" :key="point.period" :cx="x(index)" :cy="y(relativeImprovement(point.improvement.mean, baseline) ?? 0)" r="2.5">
        <title>Period {{ point.period }}: {{ relativeImprovement(point.improvement.mean, baseline) === null ? 'unavailable' : format(relativeImprovement(point.improvement.mean, baseline)!) }}</title>
      </circle>
      <text :x="inset.left" :y="height - 6">0</text>
      <text :x="width - inset.right" :y="height - 6" text-anchor="end">{{ points.at(-1)?.period ?? 0 }} periods</text>
      <text :x="inset.left - 5" :y="inset.top + 4" text-anchor="end">{{ format(domain[1]) }}</text>
      <text :x="inset.left - 5" :y="height - inset.bottom" text-anchor="end">{{ format(domain[0]) }}</text>
    </svg>
    <ol class="sr-only">
      <li v-for="point in points" :key="point.period">Period {{ point.period }}: mean improvement {{ relativeImprovement(point.improvement.mean, baseline) === null ? 'unavailable' : format(relativeImprovement(point.improvement.mean, baseline)!) }}</li>
    </ol>
  </figure>
</template>

<style scoped>
.trajectory { margin: 0; padding: var(--space-3) var(--space-4); border-top: 1px solid var(--line); background: #fbfcfa; }
.trajectory figcaption { display: flex; flex-wrap: wrap; align-items: baseline; justify-content: space-between; gap: 8px; }
.trajectory figcaption strong { font-size: var(--text-md); }
.trajectory figcaption span { color: var(--muted); font-size: var(--text-xs); }
.trajectory svg { width: 100%; aspect-ratio: 320 / 122; display: block; margin-top: 6px; overflow: visible; }
.trajectory text { fill: var(--muted); font: var(--text-2xs) var(--mono); }
.zero-line { stroke: #aeb8b2; stroke-dasharray: 3 3; }
.uncertainty-band { fill: #dbe8df; opacity: .8; }
.trajectory-line { fill: none; stroke: #6d786f; stroke-width: 2; vector-effect: non-scaling-stroke; }
.trajectory circle { fill: white; stroke: #6d786f; stroke-width: 1.5; vector-effect: non-scaling-stroke; }
.trajectory[data-impact='positive'] .trajectory-line, .trajectory[data-impact='positive'] circle { stroke: #277445; }
.trajectory[data-impact='negative'] .trajectory-line, .trajectory[data-impact='negative'] circle { stroke: #a34335; }
.trajectory[data-impact='negative'] .uncertainty-band { fill: #f1d8d2; }
</style>
