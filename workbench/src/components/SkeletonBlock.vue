<script setup lang="ts">
/**
 * A stand-in for content that is on its way.
 *
 * Sized by the caller rather than by itself, because the point of a skeleton is
 * to hold the shape of the thing it replaces: one that guessed its own size
 * would move the page under the reader the moment the real content arrived,
 * which is the jolt it exists to prevent.
 */
withDefaults(defineProps<{ height?: string; width?: string; radius?: string }>(), {
  height: '1em',
  width: '100%',
  radius: 'var(--radius-sm)',
})
</script>

<template>
  <span
    class="skeleton"
    :style="{ height, width, borderRadius: radius }"
    aria-hidden="true"
    data-test="skeleton"
  />
</template>

<style scoped>
.skeleton {
  display: block;
  flex: 0 0 auto;
  background: linear-gradient(90deg, #e4e8e1 25%, #eef1ec 50%, #e4e8e1 75%);
  background-size: 320% 100%;
  animation: sweep 1.5s ease-in-out infinite;
}

@keyframes sweep {
  from { background-position: 130% 0; }
  to { background-position: -30% 0; }
}

/*
 * A sweeping gradient reads as "working" to most people and as a distraction to
 * anyone who has asked their system to stop moving things. The block still holds
 * the space either way, which is the part that matters.
 */
@media (prefers-reduced-motion: reduce) {
  .skeleton { animation: none; background: #e4e8e1; }
}
</style>
