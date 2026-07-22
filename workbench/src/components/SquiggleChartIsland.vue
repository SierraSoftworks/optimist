<script setup lang="ts">
import type { Root } from 'react-dom/client'
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

const props = withDefaults(defineProps<{
  code: string
  label: string
  height?: number
}>(), { height: 180 })
const host = ref<HTMLElement | null>(null)
let root: Root | null = null
let revision = 0

onMounted(renderChart)
onBeforeUnmount(() => root?.unmount())
watch(() => [props.code, props.height] as const, () => void nextTick(renderChart))

async function renderChart() {
  if (!host.value || import.meta.env.MODE === 'test') return
  const current = ++revision
  const [{ createElement }, { createRoot }, { SquiggleChart }] = await Promise.all([
    import('react'),
    import('react-dom/client'),
    import('@quri/squiggle-components'),
  ])
  if (current !== revision || !host.value) return
  root ??= createRoot(host.value)
  root.render(createElement(SquiggleChart, {
    code: props.code,
    chartHeight: props.height,
    environment: { sampleCount: 2_048, xyPointLength: 200, seed: '42' },
    runner: 'embedded',
    distributionChartSettings: { showSummary: true },
  }))
}
</script>

<template>
  <div ref="host" class="squiggle-chart-island" role="img" :aria-label="label"></div>
</template>

<style scoped>
.squiggle-chart-island { min-height: 150px; overflow: hidden; border: 1px solid var(--line); border-radius: 5px; background: white; }
</style>
