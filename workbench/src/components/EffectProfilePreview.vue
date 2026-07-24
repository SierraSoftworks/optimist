<script setup lang="ts">
import { computed } from 'vue'

import { activationSeries, type EffectProfileForm } from '../domain/effectProfile'

const props = defineProps<{ form: EffectProfileForm; periods?: number }>()

const WIDTH = 320
const HEIGHT = 92
const PADDING = 6

const periods = computed(() => Math.max(4, Math.min(props.periods ?? 12, 40)))
const series = computed(() => activationSeries(props.form, periods.value))

function step(values: number[], baseline: number) {
  const width = (WIDTH - PADDING * 2) / values.length
  return values
    .map((value, index) => {
      const x = PADDING + index * width
      const y = baseline - value * (baseline - PADDING)
      return `M ${x} ${y} L ${x + width} ${y}`
    })
    .join(' ')
}

const activationPath = computed(() => step(series.value.activation, HEIGHT - PADDING))
const reboundPath = computed(() => step(series.value.rebound, HEIGHT - PADDING))
const hasRebound = computed(() => series.value.rebound.some((value) => value > 0))
const description = computed(() =>
  series.value.activation
    .map((value, index) => `period ${index + 1}: ${Math.round(value * 100)}%`)
    .join(', '),
)
</script>

<template>
  <figure class="effect-profile-preview">
    <svg :viewBox="`0 0 ${WIDTH} ${HEIGHT}`" role="img" :aria-label="`Effect strength by period. ${description}`">
      <line :x1="PADDING" :y1="HEIGHT - PADDING" :x2="WIDTH - PADDING" :y2="HEIGHT - PADDING" class="axis" />
      <path :d="activationPath" class="activation" />
      <path v-if="hasRebound" :d="reboundPath" class="rebound" />
    </svg>
    <figcaption>
      <span class="key activation-key">Effect</span>
      <span v-if="hasRebound" class="key rebound-key">Rebound</span>
      <span class="periods">{{ periods }} periods</span>
    </figcaption>
  </figure>
</template>

<style scoped>
.effect-profile-preview {
  margin: 0;
}

svg {
  width: 100%;
  height: auto;
  display: block;
  border: 1px solid var(--line);
  border-radius: 5px;
  background: var(--surface);
}

.axis {
  stroke: var(--line);
  stroke-width: 1;
}

.activation {
  fill: none;
  stroke: var(--green);
  stroke-width: 2;
  stroke-linecap: square;
}

.rebound {
  fill: none;
  stroke: var(--intervention);
  stroke-width: 2;
  stroke-dasharray: 4 3;
  stroke-linecap: square;
}

figcaption {
  display: flex;
  gap: 12px;
  align-items: center;
  margin-top: 6px;
  color: var(--muted);
  font-size: 9px;
}

.key::before {
  content: '';
  display: inline-block;
  width: 10px;
  height: 2px;
  margin-right: 5px;
  vertical-align: middle;
}

.activation-key::before {
  background: var(--green);
}

.rebound-key::before {
  background: var(--intervention);
}

.periods {
  margin-left: auto;
}
</style>
