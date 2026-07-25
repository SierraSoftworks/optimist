<script setup lang="ts">
import { computed } from 'vue'
import type { Distribution } from '../api/types'

const props = defineProps<{
  distribution: Distribution | null
  kind: 'duration' | 'probability'
}>()

const summary = computed(() => {
  const distribution = props.distribution
  if (!distribution) {
    return props.kind === 'probability'
      ? { family: 'Certain', mean: 1, low: 1, high: 1 }
      : { family: 'Immediate', mean: 0, low: 0, high: 0 }
  }
  if (distribution.type === 'point') {
    const value = distribution.value ?? 0
    return { family: 'Point', mean: value, low: value, high: value }
  }
  if (distribution.type === 'normal') {
    const mean = distribution.mean ?? 0
    const spread = (distribution.standard_deviation ?? 0) * 1.28
    return { family: 'Normal', mean, low: mean - spread, high: mean + spread }
  }
  if (distribution.type === 'log_normal') {
    const location = distribution.location ?? 0
    const scale = distribution.scale ?? 0
    return {
      family: 'LogNormal',
      mean: Math.exp(location + scale * scale / 2),
      low: Math.exp(location - 1.28 * scale),
      high: Math.exp(location + 1.28 * scale),
    }
  }
  if (distribution.type === 'empirical') {
    const values = [...(distribution.samples ?? [])].sort((left, right) => left - right)
    const mean = values.length ? values.reduce((total, value) => total + value, 0) / values.length : 0
    return {
      family: 'SampleSet', mean,
      low: values[Math.floor(values.length * 0.1)] ?? mean,
      high: values[Math.floor(values.length * 0.9)] ?? mean,
    }
  }
  const alpha = distribution.alpha ?? 1
  const beta = distribution.beta ?? 1
  const proportion = alpha / (alpha + beta)
  const lower = distribution.type === 'scaled_beta' ? distribution.lower ?? 0 : 0
  const upper = distribution.type === 'scaled_beta' ? distribution.upper ?? 1 : 1
  return {
    family: distribution.type === 'scaled_beta' ? 'Scaled Beta' : 'Beta',
    mean: lower + proportion * (upper - lower),
    low: lower,
    high: upper,
  }
})

const scale = computed(() => {
  if (props.kind === 'probability') return { low: 0, high: 1 }
  return { low: Math.min(0, summary.value.low), high: Math.max(1, summary.value.high) }
})
const position = computed(() => {
  const width = scale.value.high - scale.value.low || 1
  return `${Math.max(0, Math.min(100, (summary.value.mean - scale.value.low) / width * 100))}%`
})

function number(value: number) {
  return Number(value.toPrecision(3)).toString()
}
</script>

<template>
  <div class="distribution-strip" :data-kind="kind">
    <div class="distribution-copy">
      <strong>{{ kind === 'duration' ? `${number(summary.mean)} periods` : `${number(summary.mean * 100)}%` }}</strong>
      <span>{{ summary.family }} · visual range {{ number(summary.low) }}–{{ number(summary.high) }}</span>
    </div>
    <div class="distribution-track" aria-hidden="true"><i :style="{ left: position }"></i></div>
  </div>
</template>

<style scoped>
.distribution-strip { display: grid; gap: 5px; }
.distribution-copy { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; }
.distribution-copy strong { font: var(--text-2xs) var(--mono); }
.distribution-copy span { color: var(--muted); font-size: var(--text-2xs); }
.distribution-track { position: relative; height: 7px; overflow: hidden; border-radius: 2px; background: #dfe5df; }
.distribution-track::before { content: ''; position: absolute; inset: 0; background: linear-gradient(90deg, #d58c73, #d7c779 52%, #73a987); opacity: .65; }
.distribution-track i { position: absolute; top: -2px; width: 3px; height: 11px; border-radius: 2px; background: #17231d; transform: translateX(-50%); }
.distribution-strip[data-kind='duration'] .distribution-track::before { background: linear-gradient(90deg, #73a987, #d7c779 55%, #d58c73); }
</style>
