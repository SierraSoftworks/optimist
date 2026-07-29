<script setup lang="ts">
import { ref, watch } from 'vue'

import { api } from '../api/client'
import type { Quantity } from '../api/types'
import QuantityCard from './QuantityCard.vue'

const props = defineProps<{
  design: string
  /** Squiggle source as it currently stands, which changes on every keystroke. */
  expression: string
  /** The quantity being edited, so the preview sees only what it may refer to. */
  entry?: string | null
  unit?: string
  /** What the quantity is for, which is the thing the new expression has to keep being. */
  summary?: string
}>()

/** How long the typing has to stop before the expression is worth evaluating. */
const SETTLE_MS = 350

const quantity = ref<Quantity | null>(null)
const problem = ref<string | null>(null)
const working = ref(false)

let timer: ReturnType<typeof setTimeout> | null = null
/**
 * Which request is the current one.
 *
 * Evaluations are cheap but not instant, and a fast typist has several in flight
 * at once. Without this the preview shows whichever finished last, which for a
 * half-written expression is usually the error from two keystrokes ago.
 */
let latest = 0

watch(
  () => [props.expression, props.entry, props.design].join('\u0000'),
  () => {
    if (timer) clearTimeout(timer)
    const source = props.expression.trim()
    if (!source) {
      quantity.value = null
      problem.value = null
      return
    }
    timer = setTimeout(() => {
      const mine = (latest += 1)
      working.value = true
      api
        .preview(props.design, source, props.entry ?? null)
        .then((result) => {
          if (mine !== latest) return
          quantity.value = result
          problem.value = null
        })
        .catch((error: Error) => {
          if (mine !== latest) return
          quantity.value = null
          problem.value = error.message
        })
        .finally(() => {
          if (mine === latest) working.value = false
        })
    }, SETTLE_MS)
  },
  { immediate: true },
)
</script>

<template>
  <QuantityCard
    heading="Preview"
    :summary="summary"
    :unit="unit"
    :quantity="quantity"
    :problem="problem"
    :working="working"
  />
</template>
