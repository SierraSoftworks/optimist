<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import { api } from '../api/client'
import type { Quantity } from '../api/types'
import DistributionChart from './DistributionChart.vue'
import { formatSiNumber } from '../domain/humanNumber'

const props = defineProps<{
  design: string
  /** Squiggle source as it currently stands, which changes on every keystroke. */
  expression: string
  /** The quantity being edited, so the preview sees only what it may refer to. */
  entry?: string | null
  unit?: string
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

/** Whether this quantity is one number or a spread of them. */
const certain = computed(() => !!quantity.value && quantity.value.draws.length === 0)
</script>

<template>
  <aside class="preview" data-test="quantity-preview">
    <p class="heading">Preview</p>

    <p v-if="problem" class="problem" data-test="preview-problem">{{ problem }}</p>

    <template v-else-if="quantity">
      <!--
        A certain quantity is drawn as a number rather than as a spike. The
        spread is the whole subject of the chart, and one with no spread is a
        picture of nothing that takes a moment to recognise as such.
      -->
      <template v-if="certain">
        <p class="value" data-test="preview-value">{{ formatSiNumber(quantity.mean) }}</p>
        <p class="says">certain{{ unit && unit !== '1' ? ` — ${unit}` : '' }}</p>
      </template>
      <template v-else>
        <DistributionChart :quantity="quantity" :unit="unit" :height="72" />
        <dl class="quantiles">
          <div>
            <dt>p10</dt>
            <dd>{{ formatSiNumber(quantity.p10) }}</dd>
          </div>
          <div>
            <dt>median</dt>
            <dd>{{ formatSiNumber(quantity.p50) }}</dd>
          </div>
          <div>
            <dt>p90</dt>
            <dd>{{ formatSiNumber(quantity.p90) }}</dd>
          </div>
        </dl>
      </template>
    </template>

    <p v-else class="says">{{ working ? 'Working…' : 'Nothing to show yet.' }}</p>
  </aside>
</template>

<style scoped>
.preview {
  width: 268px;
  padding: var(--space-2) var(--space-3) var(--space-3);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface-strong);
  box-shadow: 0 8px 26px rgb(28 35 31 / 16%);
}
.heading {
  margin: 0 0 var(--space-2);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--muted);
  font-weight: 700;
}
.value { margin: 0; font-family: var(--mono); font-size: var(--text-xl); line-height: 1.1; }
.says { margin: 2px 0 0; font-size: var(--text-2xs); color: var(--muted); }
.problem {
  margin: 0;
  font-size: var(--text-2xs);
  color: var(--danger);
  font-family: var(--mono);
  line-height: 1.4;
  overflow-wrap: anywhere;
}
.quantiles { display: flex; justify-content: space-between; margin: var(--space-2) 0 0; gap: var(--space-2); }
.quantiles div { display: flex; flex-direction: column; align-items: center; }
.quantiles dt { font-size: 9px; color: var(--muted); text-transform: uppercase; letter-spacing: 0.04em; }
.quantiles dd { margin: 0; font-family: var(--mono); font-size: var(--text-2xs); }
</style>
