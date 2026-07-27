<script setup lang="ts">
import { computed } from 'vue'

import type { Quantity } from '../api/types'
import { kernelDensity } from '../domain/density'
import { formatSiNumber } from '../domain/humanNumber'

const props = withDefaults(
  defineProps<{
    quantity: Quantity
    unit?: string
    height?: number
  }>(),
  { unit: '', height: 64 },
)

const WIDTH = 260

const density = computed(() => kernelDensity(props.quantity.draws))

/**
 * The filled area under the density.
 *
 * Scaled to its own maximum rather than to a shared one: the question asked of a
 * single quantity is what shape it has, and a common vertical scale across
 * quantities measured in different units would answer nothing.
 */
const area = computed(() => {
  const estimate = density.value
  if (!estimate) return ''
  const from = estimate.x[0]
  const to = estimate.x[estimate.x.length - 1]
  const span = to - from || 1
  const tallest = Math.max(...estimate.y) || 1
  const points = estimate.x.map((x, index) => {
    const px = ((x - from) / span) * WIDTH
    const py = props.height - (estimate.y[index] / tallest) * props.height
    return `${px.toFixed(2)},${py.toFixed(2)}`
  })
  return `M0,${props.height} L${points.join(' L')} L${WIDTH},${props.height} Z`
})

/** Where a value sits horizontally within the drawn range. */
function position(value: number): number | null {
  const estimate = density.value
  if (!estimate) return null
  const from = estimate.x[0]
  const to = estimate.x[estimate.x.length - 1]
  const span = to - from || 1
  return ((value - from) / span) * WIDTH
}

const median = computed(() => position(props.quantity.p50))
const band = computed(() => {
  const low = position(props.quantity.p10)
  const high = position(props.quantity.p90)
  if (low === null || high === null) return null
  return { x: Math.min(low, high), width: Math.abs(high - low) }
})

const certain = computed(() => props.quantity.draws.length === 0 || density.value === null)
const branches = computed(() => density.value?.modes ?? 1)

function show(value: number): string {
  return formatSiNumber(value)
}
</script>

<template>
  <figure class="distribution">
    <figcaption class="sr-only">
      {{
        certain
          ? `Certain at ${show(quantity.mean)} ${unit}`
          : `Distribution from ${show(quantity.p10)} to ${show(quantity.p90)} ${unit}, ${branches} mode or modes`
      }}
    </figcaption>

    <div v-if="certain" class="certain">
      <span class="value">{{ show(quantity.mean) }}</span>
      <span v-if="unit" class="unit">{{ unit }}</span>
      <span class="note">certain</span>
    </div>

    <template v-else>
      <svg
        class="plot"
        :viewBox="`0 0 ${WIDTH} ${height}`"
        :height="height"
        preserveAspectRatio="none"
        aria-hidden="true"
      >
        <rect v-if="band" class="band" :x="band.x" :width="band.width" y="0" :height="height" />
        <path class="curve" :d="area" />
        <line v-if="median !== null" class="median" :x1="median" :x2="median" y1="0" :y2="height" />
      </svg>

      <div class="legend">
        <span class="quantile">{{ show(quantity.p10) }}</span>
        <span class="middle">
          <strong>{{ show(quantity.p50) }}</strong>
          <span v-if="unit" class="unit">{{ unit }}</span>
        </span>
        <span class="quantile">{{ show(quantity.p90) }}</span>
      </div>

      <!--
        The count is surfaced rather than left to the eye. A second branch is the
        most consequential thing a result can have, and at this size the dip
        between two modes is a few pixels.
      -->
      <p v-if="branches > 1" class="branches">
        {{ branches }} states &mdash; this quantity has settled more than one way
      </p>
    </template>
  </figure>
</template>

<style scoped>
.distribution { margin: 0; display: flex; flex-direction: column; gap: 2px; }
.plot { width: 100%; display: block; overflow: visible; }
.band { fill: var(--green-soft); }
.curve { fill: var(--green); fill-opacity: 0.5; stroke: var(--green); stroke-width: 1.25; }
.median { stroke: var(--ink); stroke-width: 1; stroke-dasharray: 2 2; }
.legend {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  font-size: var(--text-2xs);
  color: var(--muted);
  font-family: var(--mono);
}
.middle { color: var(--ink); }
.unit { color: var(--muted); margin-left: 3px; }
.certain { display: flex; align-items: baseline; gap: var(--space-2); font-family: var(--mono); }
.certain .value { font-size: var(--text-lg); }
.note { font-size: var(--text-2xs); color: var(--muted); font-family: 'Manrope', sans-serif; }
.branches {
  margin: 2px 0 0;
  font-size: var(--text-2xs);
  font-weight: 650;
  color: var(--caution);
  background: var(--caution-surface);
  border: 1px solid var(--caution-line);
  border-radius: var(--radius-sm);
  padding: 2px 6px;
}
</style>
