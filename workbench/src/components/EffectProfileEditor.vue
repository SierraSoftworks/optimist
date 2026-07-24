<script setup lang="ts">
import { computed } from 'vue'

import type { GraphEdge, Unit } from '../api/types'
import { effectProfileValid, type EffectProfileForm } from '../domain/effectProfile'
import { formatUnitExpression } from '../domain/unitExpression'
import EffectProfilePreview from './EffectProfilePreview.vue'

const props = defineProps<{ form: EffectProfileForm; edge: GraphEdge; pending: boolean }>()
const emit = defineEmits<{ save: [] }>()

const destinationUnit = computed<Unit>(() =>
  props.edge.payload.kind === 'changes' ? props.edge.payload.properties.response.destination_unit : {},
)
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
          Rebound movement ({{ formatUnitExpression(destinationUnit) }})
          <input v-model.number="form.reboundMagnitude" type="number" step="any" />
        </label>
        <label>
          Rebound holds for (periods)
          <input v-model.number="form.reboundHold" type="number" min="0" step="1" />
        </label>
      </div>
      <p class="form-note">
        The rebound is a separate movement, not a share of the effect above, because a drained
        backlog rarely returns exactly what was withheld.
      </p>
    </template>
    <EffectProfilePreview :form="form" :periods="horizon" />
    <p v-if="!valid" class="form-error">
      A time-boxed effect needs a ramp, a hold window, or a rebound, and a gradual ending needs a
      positive span.
    </p>
    <div class="dialog-actions">
      <button type="button" :disabled="pending || !valid" @click="emit('save')">
        {{ form.enabled ? 'Save profile' : 'Make permanent' }}
      </button>
    </div>
  </section>
</template>

<style scoped>
.effect-profile header {
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
  margin-bottom: 0.5rem;
}

.effect-profile header span {
  font-size: 0.78rem;
  color: var(--muted, #71717a);
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  margin-top: 0.5rem;
}
</style>
