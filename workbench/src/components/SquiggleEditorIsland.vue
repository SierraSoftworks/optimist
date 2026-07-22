<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { Root } from 'react-dom/client'

const props = withDefaults(defineProps<{
  modelValue: string
  label?: string
  sampleCount?: number
  seed?: number
}>(), {
  label: 'Squiggle source',
  sampleCount: 2_048,
  seed: 42,
})
const emit = defineEmits<{ 'update:modelValue': [source: string] }>()
const host = ref<HTMLElement | null>(null)
let root: Root | null = null
let editorSource = props.modelValue
let revision = 0

onMounted(() => renderEditor(props.modelValue))
onBeforeUnmount(() => root?.unmount())
watch(() => props.modelValue, (source) => {
  if (source === editorSource) return
  editorSource = source
  void nextTick(() => renderEditor(source))
})

async function renderEditor(source: string) {
  if (!host.value || import.meta.env.MODE === 'test') return
  const current = ++revision
  const [{ createElement }, { createRoot }, { SquiggleEditor }] = await Promise.all([
    import('react'),
    import('react-dom/client'),
    import('@quri/squiggle-components'),
  ])
  if (current !== revision || !host.value) return
  root?.unmount()
  root = createRoot(host.value)
  root.render(createElement(SquiggleEditor, {
    defaultCode: source,
    editorFontSize: 12,
    chartHeight: 210,
    environment: { sampleCount: props.sampleCount, xyPointLength: 200, seed: String(props.seed) },
    runner: 'embedded',
    onCodeChange: (value: string) => {
      editorSource = value
      emit('update:modelValue', value)
    },
  }))
}

function accessibleInput(event: Event) {
  const source = (event.target as HTMLTextAreaElement).value
  editorSource = source
  emit('update:modelValue', source)
  void nextTick(() => renderEditor(source))
}
</script>

<template>
  <div class="squiggle-react-island">
    <textarea class="sr-only" :aria-label="label" :value="modelValue" @input="accessibleInput"></textarea>
    <div ref="host" class="squiggle-react-host" aria-label="Squiggle editor and distribution viewer"></div>
  </div>
</template>

<style scoped>
.squiggle-react-island { min-width: 0; }
.squiggle-react-host { height: clamp(300px, 48vh, 430px); overflow: auto; border: 1px solid var(--line); border-radius: 5px; background: white; }
.squiggle-react-host:empty::before { content: 'Loading Squiggle editor'; display: grid; min-height: 250px; place-items: center; color: var(--muted); font-size: 9px; }
</style>
