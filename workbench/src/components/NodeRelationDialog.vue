<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { X } from '@lucide/vue'

import type { GraphEdge, GraphNode, SetStateRelationInput } from '../api/types'
import {
  canOwnRelation,
  nodeQuantity,
  nodeRelation,
  relationBindings,
} from '../domain/stateRelation'
import { formatUnitExpression } from '../domain/unitExpression'

const props = defineProps<{
  open: boolean
  pending: boolean
  node: GraphNode | null
  nodes: GraphNode[]
  edges: GraphEdge[]
}>()
const emit = defineEmits<{ close: []; submit: [input: SetStateRelationInput] }>()

const source = ref('')

const bindings = computed(() =>
  props.node ? relationBindings(props.node, props.nodes, props.edges) : [],
)
const resultUnit = computed(() => {
  const dimension = props.node ? nodeQuantity(props.node)?.dimension : undefined
  return dimension === undefined ? 'no declared unit' : formatUnitExpression(dimension) || '1'
})
const existing = computed(() => (props.node ? nodeRelation(props.node) : null))
const parameterNames = computed(() => Object.keys(existing.value?.parameters ?? {}))
const eligible = computed(() => Boolean(props.node && canOwnRelation(props.node)))

watch(
  () => [props.open, props.node] as const,
  ([open]) => {
    if (!open) return
    source.value = existing.value?.source ?? ''
  },
  { immediate: true },
)

function submit() {
  const trimmed = source.value.trim()
  if (!trimmed) return
  // Parameters are not editable here yet, so they are carried through unchanged
  // rather than silently dropped along with the source they belong to.
  emit('submit', {
    relation: { source: trimmed, parameters: existing.value?.parameters },
  })
}

function clear() {
  emit('submit', { relation: null })
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open && node" class="dialog-backdrop" @pointerdown.self="emit('close')">
      <form class="dialog relation-dialog" aria-labelledby="node-relation-title" @submit.prevent="submit">
        <header>
          <div>
            <span class="eyebrow">{{ node.title }}</span>
            <h2 id="node-relation-title">Node equation</h2>
          </div>
          <button type="button" class="icon-button" aria-label="Close" @click="emit('close')"><X :size="18" /></button>
        </header>

        <p class="dialog-note">
          An equation computes this state from its parents each period, replacing the proportional
          responses on the relationships reaching it. Those relationships still decide which parents
          exist and how far they lag.
        </p>

        <template v-if="eligible">
          <label>
            Calculation
            <textarea
              v-model="source"
              rows="6"
              spellcheck="false"
              placeholder="outage_frequency * impact_duration"
            ></textarea>
          </label>
          <p class="result-unit">Must produce <strong>{{ resultUnit }}</strong></p>

          <section class="bindings">
            <h3>Names you can use</h3>
            <ul>
              <li v-for="binding of bindings" :key="`${binding.kind}-${binding.name}`">
                <code>{{ binding.name }}</code>
                <span class="binding-unit">{{ binding.unit }}</span>
                <span class="binding-kind">{{
                  binding.kind === 'baseline'
                    ? 'this state before interventions'
                    : binding.kind === 'activation'
                      ? `${binding.title} activation, 0 to 1`
                      : binding.title
                }}</span>
              </li>
            </ul>
            <p v-if="parameterNames.length" class="binding-parameters">
              Parameters: {{ parameterNames.map((name) => name).join(', ') }}
            </p>
            <p class="binding-hint">
              Uncertainty belongs in a named parameter, not in the calculation: a parameter is drawn
              once per simulation and held across the horizon, while a distribution written here
              would be redrawn every period.
            </p>
          </section>

          <footer>
            <button
              v-if="existing"
              type="button"
              class="secondary-button"
              :disabled="pending"
              @click="clear"
            >
              Remove equation
            </button>
            <button type="button" class="secondary-button" @click="emit('close')">Cancel</button>
            <button type="submit" class="primary-button" :disabled="pending || !source.trim()">
              {{ pending ? 'Saving…' : existing ? 'Replace equation' : 'Add equation' }}
            </button>
          </footer>
        </template>
        <template v-else>
          <p class="form-error">
            Configure a canonical quantity for this node before writing an equation, so the
            calculation has a unit to produce.
          </p>
          <footer>
            <button type="button" class="secondary-button" @click="emit('close')">Close</button>
          </footer>
        </template>
      </form>
    </div>
  </Teleport>
</template>

<style scoped>
.relation-dialog { width: min(560px, 92vw); }
.dialog-note { margin: 0; color: var(--muted); font-size: var(--text-sm); line-height: 1.55; }
.relation-dialog textarea { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: var(--text-sm); }
.result-unit { margin: -4px 0 0; color: var(--muted); font-size: var(--text-sm); }
.bindings { display: grid; gap: 6px; padding: 10px; border: 1px solid var(--line); border-radius: 6px; background: #fbfbfa; }
.bindings h3 { margin: 0; font-size: var(--text-sm); }
.bindings ul { display: grid; gap: 4px; margin: 0; padding: 0; list-style: none; }
.bindings li { display: flex; align-items: baseline; gap: 8px; font-size: var(--text-sm); }
.bindings code { padding: 1px 5px; border: 1px solid var(--line); border-radius: 4px; background: white; }
.binding-unit { color: var(--ink); font-size: var(--text-2xs); }
.binding-kind { color: var(--muted); font-size: var(--text-2xs); }
.binding-parameters, .binding-hint { margin: 0; color: var(--muted); font-size: var(--text-2xs); line-height: 1.5; }
</style>
