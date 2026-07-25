<script setup lang="ts">
import { computed } from 'vue'

import type { GraphEdge } from '../api/types'
import { effectProfileValid, type EffectProfileForm } from '../domain/effectProfile'
import EffectProfilePreview from './EffectProfilePreview.vue'

const props = defineProps<{ form: EffectProfileForm; edge: GraphEdge; pending: boolean }>()
const emit = defineEmits<{ save: [] }>()

const valid = computed(() => effectProfileValid(props.form))
const horizon = computed(
  () => props.form.ramp + props.form.hold + props.form.reboundHold + props.form.releaseSpan + 3,
)
</script>

<template>
  <section class="dialog-section effect-profile">
    <header>
      <strong>Effect profile</strong>
      <span>
        Model how long this intervention holds and what happens when it ends. A permanent effect
        needs no profile.
      </span>
    </header>
    <label class="checkbox-label">
      <input v-model="form.enabled" type="checkbox" />
      Time-box this intervention
    </label>
    <template v-if="form.enabled">
      <div class="field-grid">
        <label>
          Ramp (periods)
          <input v-model.number="form.ramp" type="number" min="0" step="1" />
        </label>
        <label>
          Hold (periods)
          <input v-model.number="form.hold" type="number" min="0" step="1" />
        </label>
      </div>
      <div class="field-grid">
        <label>
          Ends
          <select v-model="form.release">
            <option value="immediate">Abruptly</option>
            <option value="linear">Declining to zero</option>
            <option value="exponential">Decaying by half-life</option>
          </select>
        </label>
        <label v-if="form.release !== 'immediate'">
          {{ form.release === 'linear' ? 'Decline over (periods)' : 'Half-life (periods)' }}
          <input v-model.number="form.releaseSpan" type="number" min="1" step="1" />
        </label>
      </div>
      <label class="checkbox-label">
        <input v-model="form.reboundEnabled" type="checkbox" />
        Ending this intervention has its own effect
      </label>
      <div v-if="form.reboundEnabled" class="field-grid">
        <label>
          Rebound multiplier
          <input v-model.number="form.reboundMagnitude" type="number" step="any" />
        </label>
        <label>
          Rebound holds for (periods)
          <input v-model.number="form.reboundHold" type="number" min="0" step="1" />
        </label>
      </div>
      <p class="form-note">
        The rebound is a separate multiplier, not a share of the effect above, because a drained
        backlog rarely returns exactly what was withheld. 1 leaves the target at baseline.
      </p>
    </template>
    <EffectProfilePreview :form="form" :periods="horizon" />
    <p v-if="!valid" class="form-error">
      A time-boxed effect needs a ramp, a hold window, or a rebound, and a gradual ending needs a
      positive span.
    </p>
    <div class="dialog-actions">
      <button type="button" class="secondary-button" :disabled="pending || !valid" @click="emit('save')">
        {{ form.enabled ? 'Save profile' : 'Make permanent' }}
      </button>
    </div>
  </section>
</template>

<style scoped>
.effect-profile > header {
  display: grid;
  gap: 2px;
  margin-bottom: 12px;
}

.effect-profile > header strong {
  font-size: 11px;
}

.effect-profile > header span {
  color: var(--muted);
  font-size: 9px;
  line-height: 1.45;
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  margin-top: 12px;
}
</style>
