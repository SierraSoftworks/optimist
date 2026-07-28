<script setup lang="ts">
import SkeletonBlock from './SkeletonBlock.vue'

/**
 * The shape of a chart that has not been solved yet.
 *
 * Laid out like [`MetricTimeline`](./MetricTimeline.vue) rather than as a
 * generic box, so that the answer arriving swaps one figure for another instead
 * of pushing everything below it down the page. The plot area is given that
 * chart's aspect rather than a height in pixels, because the real one is an SVG
 * that takes its height from whatever width the column happens to have.
 */
</script>

<template>
  <figure class="timeline" aria-busy="true" data-test="chart-skeleton">
    <figcaption>
      <SkeletonBlock width="120px" height="13px" />
      <SkeletonBlock width="64px" height="11px" />
    </figcaption>
    <div class="plot">
      <SkeletonBlock height="100%" />
    </div>
  </figure>
</template>

<style scoped>
.timeline {
  margin: 0;
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface-strong);
  padding: var(--space-3);
}
figcaption {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  margin-bottom: var(--space-1);
}
/* The viewBox MetricTimeline draws into, so the two occupy the same space. */
.plot { aspect-ratio: 640 / 120; }
</style>
