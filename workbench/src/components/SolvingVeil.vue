<script setup lang="ts">
/**
 * Covers results that are no longer about the question on screen.
 *
 * When a variant is chosen the previous answer stays mounted while the new one
 * is solved, because throwing away a nearly-right chart makes the page flash
 * and tells the reader nothing. What it must never do is read as current: the
 * numbers underneath belong to a different variant, and somebody glancing at
 * them would draw a conclusion about a design nobody solved.
 *
 * So the content is blurred and drained of colour rather than merely dimmed.
 * A label alone would not do — labels are read after the chart, if at all —
 * whereas figures that cannot be focused on are not mistaken for figures.
 */
withDefaults(defineProps<{ busy: boolean; label?: string }>(), { label: 'Solving' })
</script>

<template>
  <div class="veiled">
    <div class="stack" :class="{ stale: busy }" :aria-busy="busy || undefined">
      <slot />
    </div>
    <div v-if="busy" class="veil" data-test="solving-veil">
      <span class="badge" role="status">
        <el-icon class="spinner"><i-loading /></el-icon>
        <span>{{ label }}</span>
      </span>
    </div>
  </div>
</template>

<style scoped>
.veiled { position: relative; min-width: 0; }
.stack {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-width: 0;
}
.stale {
  /*
   * Enough blur that a number cannot be read off it. Anything gentler and the
   * chart still looks legible from a metre away, which is exactly the distance
   * at which somebody decides they already know what it says.
   */
  filter: blur(3px) grayscale(0.9);
  opacity: 0.5;
  transition: filter 120ms ease-out, opacity 120ms ease-out;
  pointer-events: none;
  user-select: none;
}
.veil {
  position: absolute;
  inset: 0;
  display: flex;
  justify-content: center;
  align-items: flex-start;
  pointer-events: none;
}
.badge {
  position: sticky;
  top: var(--space-4);
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  margin-top: var(--space-4);
  padding: 5px var(--space-3);
  border: 1px solid var(--line);
  border-radius: 999px;
  background: var(--surface-strong);
  box-shadow: 0 4px 16px rgb(28 35 31 / 16%);
  font-size: var(--text-xs);
  font-weight: 650;
  color: var(--ink);
  white-space: nowrap;
}
.spinner { color: var(--green); animation: turn 900ms linear infinite; }

@keyframes turn {
  to { transform: rotate(360deg); }
}

@media (prefers-reduced-motion: reduce) {
  .spinner { animation: none; }
  .stale { transition: none; }
}
</style>
