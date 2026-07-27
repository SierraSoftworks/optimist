<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import type { MonteCarloEstimate } from '../api/types'
import { outcomeScale, positionOn } from '../domain/outcomeScale'
import { formatSiNumber } from '../domain/humanNumber'

/**
 * One propagated state's path, drawn small enough to read a whole model at once.
 *
 * This is a debugging surface rather than a decision one: no direction, no
 * comparison run, no colour carrying a judgement. A state is neither good nor
 * bad, and the question being asked of it is where its value stopped making
 * sense.
 */
const props = defineProps<{
  points: MonteCarloEstimate[]
  label: string
  unit: string | null
}>()

const host = ref<HTMLElement | null>(null)
const width = ref(220)
const height = 62
const inset = { top: 8, right: 4, bottom: 8, left: 40 }
const usableWidth = computed(() => width.value - inset.left - inset.right)
const usableHeight = height - inset.top - inset.bottom

let observer: ResizeObserver | null = null
onMounted(() => {
  if (!host.value || typeof ResizeObserver === 'undefined') return
  observer = new ResizeObserver(([entry]) => {
    const measured = entry?.contentRect.width ?? 0
    if (measured > 0) width.value = Math.round(measured)
  })
  observer.observe(host.value)
})
onBeforeUnmount(() => observer?.disconnect())

const means = computed(() => props.points.map((point) => point.mean))
const scale = computed(() => outcomeScale(means.value))
const line = computed(() => means.value
  .flatMap((value, index) => value === null
    ? []
    : [`${index ? 'L' : 'M'} ${x(index)} ${y(value)}`])
  .join(' '))

/**
 * Whether the state ever moves.
 *
 * A path that never leaves its starting value is usually the interesting one
 * when a projection looks wrong, so it is called out rather than left as a flat
 * line among dozens of others.
 */
const inert = computed(() => {
  const values = means.value.filter((value): value is number => value !== null)
  return values.length > 1 && values.every((value) => value === values[0])
})

function x(index: number) {
  const count = props.points.length
  return inset.left + (count <= 1 ? 0 : (index / (count - 1)) * usableWidth.value)
}

function y(value: number) {
  return inset.top + (1 - positionOn(scale.value, value)) * usableHeight
}

function format(value: number | null) {
  return value === null ? '—' : formatSiNumber(value)
}
</script>

<template>
  <figure class="state-trace" :data-inert="inert" :aria-label="`${label} over time`">
    <figcaption>
      <span class="state-name">{{ label }}</span>
      <span class="state-range">
        {{ format(scale.lower) }}–{{ format(scale.upper) }}<template v-if="unit"> {{ unit }}</template>
      </span>
    </figcaption>
    <div ref="host" class="plot">
      <svg :viewBox="`0 0 ${width} ${height}`" :style="{ height: `${height}px` }" role="img">
        <path v-if="line" :d="line" class="trace-line" />
        <text x="0" :y="inset.top + 4">{{ format(means.at(-1) ?? null) }}</text>
        <text x="0" :y="height - inset.bottom">{{ format(means[0] ?? null) }}</text>
      </svg>
    </div>
    <ol class="sr-only">
      <li v-for="(value, index) in means" :key="index">Period {{ index }}: {{ format(value) }}</li>
    </ol>
  </figure>
</template>

<style scoped>
.state-trace { margin: 0; padding: var(--space-2); border: 1px solid var(--line); border-radius: var(--radius-sm); background: white; }
.state-trace[data-inert='true'] { border-style: dashed; opacity: 0.72; }
figcaption { display: flex; align-items: baseline; justify-content: space-between; gap: var(--space-2); }
.state-name { min-width: 0; overflow: hidden; font-size: var(--text-xs); font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.state-range { flex: none; color: var(--muted); font: var(--text-2xs) var(--mono); }
.plot { min-width: 0; }
svg { display: block; width: 100%; overflow: visible; }
text { fill: var(--muted); font-size: var(--text-2xs); font-family: var(--mono); }
.trace-line { fill: none; stroke: #3f6f8f; stroke-width: 1.5; }
</style>
