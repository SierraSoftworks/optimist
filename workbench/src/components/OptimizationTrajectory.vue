<script setup lang="ts">
import { computed } from 'vue'
import type { ObjectiveTrajectoryPoint } from '../api/types'

const props = defineProps<{
  points: ObjectiveTrajectoryPoint[]
  label: string
}>()

const width = 320
const height = 122
const inset = { top: 12, right: 12, bottom: 24, left: 34 }
const usableWidth = width - inset.left - inset.right
const usableHeight = height - inset.top - inset.bottom
const values = computed(() => props.points.flatMap((point) => {
  const mean = point.improvement.mean
  if (mean === null) return []
  const spread = point.improvement.variance === null ? 0 : Math.sqrt(Math.max(0, point.improvement.variance))
  return [mean - spread, mean + spread, 0]
}))
const domain = computed(() => {
  const minimum = Math.min(...values.value, 0)
  const maximum = Math.max(...values.value, 0)
  const padding = Math.max((maximum - minimum) * 0.12, 0.01)
  return [minimum - padding, maximum + padding] as const
})
const line = computed(() => path(props.points.map((point) => point.improvement.mean)))
const band = computed(() => {
  const upper = props.points.map((point) => bound(point, 1))
  const lower = props.points.map((point) => bound(point, -1)).reverse()
  return `${pathPoints(upper)} ${pathPoints(lower, true)}`
})
const zeroY = computed(() => y(0))

function x(index: number) {
  return inset.left + (props.points.length <= 1 ? 0 : index / (props.points.length - 1) * usableWidth)
}

function y(value: number) {
  const [minimum, maximum] = domain.value
  return inset.top + (maximum - value) / (maximum - minimum) * usableHeight
}

function bound(point: ObjectiveTrajectoryPoint, direction: number) {
  const mean = point.improvement.mean ?? 0
  const spread = point.improvement.variance === null ? 0 : Math.sqrt(Math.max(0, point.improvement.variance))
  return mean + direction * spread
}

function path(points: Array<number | null>) {
  return points.flatMap((value, index) => value === null ? [] : [`${index ? 'L' : 'M'} ${x(index)} ${y(value)}`]).join(' ')
}

function pathPoints(points: number[], continuePath = false) {
  return points.map((value, index) => `${continuePath || index ? 'L' : 'M'} ${x(continuePath ? points.length - index - 1 : index)} ${y(value)}`).join(' ')
}

function format(value: number) {
  return Number(value.toPrecision(3)).toString()
}
</script>

<template>
  <figure class="trajectory" :aria-label="`${label} improvement over time`">
    <figcaption><strong>{{ label }}</strong><span>Mean shift with ±1 SD</span></figcaption>
    <svg :viewBox="`0 0 ${width} ${height}`" role="img">
      <line :x1="inset.left" :x2="width - inset.right" :y1="zeroY" :y2="zeroY" class="zero-line" />
      <path v-if="points.length" :d="`${band} Z`" class="uncertainty-band" />
      <path v-if="points.length" :d="line" class="trajectory-line" />
      <circle v-for="(point, index) in points" :key="point.period" :cx="x(index)" :cy="y(point.improvement.mean ?? 0)" r="2.5">
        <title>Period {{ point.period }}: {{ point.improvement.mean === null ? 'unavailable' : format(point.improvement.mean) }}</title>
      </circle>
      <text :x="inset.left" :y="height - 6">0</text>
      <text :x="width - inset.right" :y="height - 6" text-anchor="end">{{ points.at(-1)?.period ?? 0 }} periods</text>
      <text :x="inset.left - 5" :y="inset.top + 4" text-anchor="end">{{ format(domain[1]) }}</text>
      <text :x="inset.left - 5" :y="height - inset.bottom" text-anchor="end">{{ format(domain[0]) }}</text>
    </svg>
    <ol class="sr-only">
      <li v-for="point in points" :key="point.period">Period {{ point.period }}: mean improvement {{ point.improvement.mean }}</li>
    </ol>
  </figure>
</template>

<style scoped>
.trajectory { margin: 0; padding: 9px; border-top: 1px solid var(--line); background: #fbfcfa; }
.trajectory figcaption { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; }
.trajectory figcaption strong { font-size: 9px; }
.trajectory figcaption span { color: var(--muted); font-size: 7px; }
.trajectory svg { width: 100%; aspect-ratio: 320 / 122; display: block; margin-top: 4px; overflow: visible; }
.trajectory text { fill: var(--muted); font: 7px 'IBM Plex Mono', monospace; }
.zero-line { stroke: #aeb8b2; stroke-dasharray: 3 3; }
.uncertainty-band { fill: #dbe8df; opacity: .8; }
.trajectory-line { fill: none; stroke: var(--green); stroke-width: 2; vector-effect: non-scaling-stroke; }
.trajectory circle { fill: white; stroke: var(--green); stroke-width: 1.5; vector-effect: non-scaling-stroke; }
</style>
