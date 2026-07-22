<script setup lang="ts">
import { computed } from 'vue'
import ParameterHelp from './ParameterHelp.vue'
import type { Distribution } from '../api/types'
import { distributionPreview } from '../domain/distributionPreview'

const props = defineProps<{
  distribution: Distribution
  domain?: [number, number]
}>()
const model = computed(() => distributionPreview(props.distribution, props.domain))
const width = 360
const height = 112
const padding = { top: 12, right: 12, bottom: 25, left: 12 }
const plotWidth = width - padding.left - padding.right
const plotHeight = height - padding.top - padding.bottom
const linePath = computed(() => model.value.density.map((point, index) => {
  const x = padding.left + index / (model.value.density.length - 1) * plotWidth
  const y = padding.top + (1 - point.density) * plotHeight
  return `${index === 0 ? 'M' : 'L'} ${x.toFixed(2)} ${y.toFixed(2)}`
}).join(' '))
const areaPath = computed(() => linePath.value
  ? `${linePath.value} L ${width - padding.right} ${padding.top + plotHeight} L ${padding.left} ${padding.top + plotHeight} Z`
  : '')
const markerX = computed(() => {
  if (model.value.marker === null) return 0
  const [lower, upper] = model.value.domain
  return padding.left + (model.value.marker - lower) / (upper - lower) * plotWidth
})

function format(value: number) {
  const magnitude = Math.abs(value)
  if (magnitude !== 0 && (magnitude >= 10_000 || magnitude < 0.001)) return value.toExponential(2)
  return Number(value.toPrecision(4)).toString()
}
</script>

<template>
  <section class="distribution-preview" :aria-label="`${model.family} distribution preview`">
    <div class="distribution-preview-header">
      <div><strong>{{ model.family }}</strong><span>{{ model.support }}</span></div>
      <ParameterHelp
        label="Reading this chart"
        text="Higher parts of the curve are more plausible, not more valuable. The horizontal axis shows possible values; the total shaded area represents all modeled uncertainty."
      />
    </div>
    <svg :viewBox="`0 0 ${width} ${height}`" role="img" :aria-label="model.summary">
      <line :x1="padding.left" :x2="width - padding.right" :y1="padding.top + plotHeight" :y2="padding.top + plotHeight" class="preview-axis" />
      <path v-if="areaPath" :d="areaPath" class="preview-area" />
      <path v-if="linePath" :d="linePath" class="preview-line" />
      <g v-else>
        <line :x1="markerX" :x2="markerX" :y1="padding.top" :y2="padding.top + plotHeight" class="preview-marker" />
        <circle :cx="markerX" :cy="padding.top + 5" r="4" class="preview-marker-dot" />
      </g>
      <text :x="padding.left" :y="height - 7" text-anchor="start">{{ format(model.domain[0]) }}</text>
      <text :x="width - padding.right" :y="height - 7" text-anchor="end">{{ format(model.domain[1]) }}</text>
    </svg>
    <p>{{ model.summary }}</p>
  </section>
</template>

<style scoped>
.distribution-preview { padding: 11px 12px 10px; border: 1px solid var(--line); border-radius: 6px; background: #f7f9f5; }
.distribution-preview-header { display: flex; align-items: flex-start; justify-content: space-between; gap: 10px; }
.distribution-preview-header > div { display: grid; gap: 2px; }
.distribution-preview-header strong { color: var(--ink); font-size: 11px; }
.distribution-preview-header span { color: var(--muted); font-size: 9px; }
.distribution-preview svg { display: block; width: 100%; height: 112px; margin-top: 4px; overflow: visible; }
.distribution-preview svg text { fill: var(--muted); font: 9px 'IBM Plex Mono', monospace; }
.preview-axis { stroke: #aeb6b0; stroke-width: 1; }
.preview-area { fill: #9bc5b2; fill-opacity: .42; }
.preview-line { fill: none; stroke: var(--green); stroke-width: 2; vector-effect: non-scaling-stroke; }
.preview-marker { stroke: var(--green); stroke-width: 2.5; vector-effect: non-scaling-stroke; }
.preview-marker-dot { fill: var(--green); }
.distribution-preview > p { margin: 2px 0 0; color: #4f5954; font-size: 10px; line-height: 1.5; }
</style>
