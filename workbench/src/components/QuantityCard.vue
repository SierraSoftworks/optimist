<script setup lang="ts">
import { computed } from 'vue'

import type { Quantity } from '../api/types'
import DistributionChart from './DistributionChart.vue'
import { scaleFor, showScaled } from '../domain/units'

const props = withDefaults(
  defineProps<{
    /** The small label above the card, naming what the figure belongs to. */
    heading: string
    /** The quantity's own name, where the heading does not already say it. */
    subject?: string
    /** What the quantity is for, in the words of whoever defined it. */
    summary?: string
    unit?: string
    quantity?: Quantity | null
    /** Why there is no quantity to draw, where something went wrong. */
    problem?: string | null
    /** Whether an answer is on its way, which is not the same as there being none. */
    working?: boolean
  }>(),
  { subject: '', summary: '', unit: '', quantity: null, problem: null, working: false },
)

/** Whether this quantity is one number or a spread of them. */
const certain = computed(() => !!props.quantity && props.quantity.draws.length === 0)

/** How to read this quantity, which its declaration settles. */
const scale = computed(() => scaleFor(props.unit))

function show(value: number): string {
  return showScaled(value, scale.value)
}
</script>

<template>
  <aside class="preview" data-test="quantity-preview">
    <p class="heading">{{ heading }}</p>
    <p v-if="subject" class="subject">{{ subject }}</p>

    <p v-if="summary" class="about" data-test="preview-summary">{{ summary }}</p>

    <p v-if="problem" class="problem" data-test="preview-problem">{{ problem }}</p>

    <template v-else-if="quantity">
      <!--
        A certain quantity is drawn as a number rather than as a spike. The
        spread is the whole subject of the chart, and one with no spread is a
        picture of nothing that takes a moment to recognise as such.
      -->
      <template v-if="certain">
        <p class="value" data-test="preview-value">{{ show(quantity.mean) }}</p>
        <p class="says">certain{{ scale.suffix && scale.suffix !== '%' ? ` — ${scale.suffix}` : '' }}</p>
      </template>
      <template v-else>
        <DistributionChart :quantity="quantity" :unit="unit" :height="72" />
        <dl class="quantiles">
          <div>
            <dt>p10</dt>
            <dd>{{ show(quantity.p10) }}</dd>
          </div>
          <div>
            <dt>median</dt>
            <dd>{{ show(quantity.p50) }}</dd>
          </div>
          <div>
            <dt>p90</dt>
            <dd>{{ show(quantity.p90) }}</dd>
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
  font-family: var(--display);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--muted);
  font-weight: 700;
}
.subject {
  margin: -6px 0 var(--space-2);
  font-family: var(--mono);
  font-size: var(--text-2xs);
  color: var(--ink);
  overflow-wrap: anywhere;
}
.value { margin: 0; font-family: var(--mono); font-size: var(--text-xl); line-height: 1.1; }
.about {
  margin: 0 0 var(--space-2);
  padding-bottom: var(--space-2);
  border-bottom: 1px solid var(--line);
  font-size: var(--text-2xs);
  line-height: 1.45;
  color: var(--muted);
  white-space: pre-wrap;
}
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
