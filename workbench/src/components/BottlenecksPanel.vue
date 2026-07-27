<script setup lang="ts">
import { computed } from 'vue'

import type { Analysis } from '../api/types'
import { formatSiNumber } from '../domain/humanNumber'
import DistributionChart from './DistributionChart.vue'

const props = defineProps<{ analysis: Analysis }>()

/**
 * Constraints worst first, as the server ranked them.
 *
 * The order is not re-derived here. Ranking is a modelling judgement and belongs
 * where the model is.
 */
const ranked = computed(() => props.analysis.bottlenecks)

function share(value: number): string {
  return `${(value * 100).toFixed(0)}%`
}
</script>

<template>
  <section class="bottlenecks">
    <p v-if="!analysis.converged" class="warning">
      This design did not settle after {{ analysis.iterations }} passes. A loop whose gain exceeds
      one has no steady state, so the figures below are wherever the solver stopped rather than
      what the design does.
    </p>

    <p v-if="ranked.length === 0" class="empty">
      Nothing in this design declares a limit, so there is nothing to exhaust.
    </p>

    <table v-else class="ranking">
      <thead>
        <tr>
          <th scope="col">Constraint</th>
          <th scope="col" class="numeric">Utilisation</th>
          <th scope="col" class="numeric">p90</th>
          <th scope="col" class="numeric">Binds</th>
          <th scope="col" class="numeric">Headroom</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="entry in ranked" :key="`${entry.component}/${entry.constraint}`">
          <th scope="row" class="what">
            <span class="component">{{ entry.component }}</span>
            <span class="constraint">{{ entry.constraint }}</span>
            <span class="summary">{{ entry.summary }}</span>
            <span v-if="entry.replicas > 1" class="replicas">
              across {{ formatSiNumber(entry.replicas) }} replicas
            </span>
          </th>
          <td class="numeric" :class="{ over: entry.utilisation >= 1 }">
            {{ entry.utilisation.toFixed(3) }}
          </td>
          <td class="numeric">{{ entry.utilisation_p90.toFixed(3) }}</td>
          <!--
            The share of draws that bind is reported next to the mean because
            they routinely disagree: a mean under one with a large share binding
            is a design that fails often and looks safe on average.
          -->
          <td class="numeric" :class="{ over: entry.probability_of_binding > 0 }">
            {{ share(entry.probability_of_binding) }}
          </td>
          <td class="numeric">{{ formatSiNumber(entry.headroom) }}</td>
        </tr>
      </tbody>
    </table>

    <div v-if="Object.keys(analysis.components).length" class="quantities">
      <h3>Solved quantities</h3>
      <div class="grid">
        <article v-for="(channels, component) in analysis.components" :key="component" class="card">
          <h4>{{ component }}</h4>
          <div v-for="(quantity, channel) in channels" :key="channel" class="channel">
            <span class="name">{{ channel }}</span>
            <DistributionChart :quantity="quantity" :height="44" />
          </div>
          <p v-if="!Object.keys(channels).length" class="empty">No channels.</p>
        </article>
      </div>
    </div>
  </section>
</template>

<style scoped>
.bottlenecks { display: flex; flex-direction: column; gap: var(--space-5); }
.warning {
  margin: 0;
  padding: var(--space-3);
  border: 1px solid var(--caution-line);
  background: var(--caution-surface);
  color: var(--caution);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
}
.empty { color: var(--muted); font-size: var(--text-sm); margin: 0; }
.ranking { width: 100%; border-collapse: collapse; font-size: var(--text-sm); }
.ranking th, .ranking td { text-align: left; padding: var(--space-2) var(--space-3); border-bottom: 1px solid var(--line); vertical-align: top; }
.numeric { text-align: right; font-family: var(--mono); white-space: nowrap; }
.over { color: var(--danger); font-weight: 650; }
.what { display: flex; flex-direction: column; gap: 2px; font-weight: 400; }
.component { font-weight: 700; }
.constraint { font-family: var(--mono); font-size: var(--text-xs); color: var(--muted); }
.summary { color: var(--muted); font-size: var(--text-xs); max-width: 52ch; }
.replicas { font-size: var(--text-2xs); color: var(--muted); }
.quantities h3 { font-size: var(--text-md); margin: 0 0 var(--space-3); }
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: var(--space-4); }
.card { border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface); padding: var(--space-3); }
.card h4 { margin: 0 0 var(--space-2); font-size: var(--text-sm); font-family: var(--mono); }
.channel { padding: var(--space-2) 0; border-top: 1px solid var(--line); }
.channel .name { font-size: var(--text-2xs); color: var(--muted); font-family: var(--mono); }
</style>
