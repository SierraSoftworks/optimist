<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Link2, Link2Off, TriangleAlert } from '@lucide/vue'

import type { EstimateAddress, ProjectDependenceModel } from '../api/types'
import type { CatalogueEntry } from '../domain/estimateCatalogue'
import { partnersOf, sameAddress } from '../domain/estimateCoupling'

const props = defineProps<{
  /** Address of the estimate being edited, absent until its owner exists. */
  address: EstimateAddress | null
  /** Canonical unit text this estimate must agree with to share a quantity. */
  unit: string
  /** Squiggle source currently in the editor, compared against its partners. */
  source: string
  catalogue: CatalogueEntry[]
  dependence: ProjectDependenceModel | null
  pending: boolean
}>()
const emit = defineEmits<{
  /** Couple with the chosen estimate and adopt its authored source. */
  share: [partner: CatalogueEntry]
  unshare: []
}>()

const chosen = ref('')

const partners = computed(() => {
  if (!props.address) return []
  return partnersOf(props.dependence, props.address).map((member) => ({
    member,
    entry: props.catalogue.find((item) => sameAddress(item.address, member)) ?? null,
  }))
})

/**
 * Estimates offered as the same quantity.
 *
 * A shared quantity is one variable measured once, so only estimates carrying
 * the same canonical unit are offered. Coupling estimates in different units is
 * meaningful — they take the same quantile of their own distribution — but it
 * is a correlation claim rather than an identity, so it belongs in the
 * dependence document rather than behind this button.
 */
const candidates = computed(() =>
  props.catalogue.filter(
    (entry) =>
      props.address &&
      entry.unit === props.unit &&
      !sameAddress(entry.address, props.address) &&
      !partners.value.some((partner) => sameAddress(partner.member, entry.address)),
  ),
)

const divergent = computed(() =>
  partners.value.filter(
    (partner) => partner.entry && partner.entry.source.trim() !== props.source.trim(),
  ),
)

watch(candidates, (entries) => {
  if (!entries.some((entry) => key(entry.address) === chosen.value)) chosen.value = ''
})

function key(address: EstimateAddress) {
  const owner =
    address.owner.kind === 'node'
      ? address.owner.id
      : `${address.owner.id.source}-${address.owner.id.kind}-${address.owner.id.destination}`
  return `${address.owner.kind}/${owner}/${address.estimate}`
}

function share() {
  const entry = candidates.value.find((item) => key(item.address) === chosen.value)
  if (entry) emit('share', entry)
}
</script>

<template>
  <section v-if="address" class="shared-quantity">
    <header>
      <strong>Shared quantity</strong>
      <span>
        Two estimates standing for the same real quantity should move together. Sharing couples
        them at a correlation of one, so every simulated draw gives both the same value.
      </span>
    </header>

    <ul v-if="partners.length" class="partner-list">
      <li v-for="partner of partners" :key="key(partner.member)">
        <Link2 :size="13" aria-hidden="true" />
        <span>{{ partner.entry?.label ?? key(partner.member) }}</span>
      </li>
    </ul>

    <p v-if="divergent.length" class="form-warning">
      <TriangleAlert :size="13" aria-hidden="true" />
      This definition differs from
      {{ divergent.map((partner) => partner.entry?.label).join(', ') }}. Coupled estimates with
      different definitions take the same quantile rather than the same value.
    </p>

    <div v-if="partners.length" class="dialog-actions">
      <button type="button" class="secondary-button" :disabled="pending" @click="emit('unshare')">
        <Link2Off :size="13" aria-hidden="true" />
        Stop sharing
      </button>
    </div>

    <template v-else>
      <label v-if="candidates.length">
        Same quantity as
        <select v-model="chosen">
          <option value="">Choose an estimate…</option>
          <option v-for="entry of candidates" :key="key(entry.address)" :value="key(entry.address)">
            {{ entry.label }}
          </option>
        </select>
      </label>
      <p v-else class="form-note">
        No other estimate in this project is measured in {{ unit || 'the same unit' }} yet.
      </p>
      <div v-if="candidates.length" class="dialog-actions">
        <button
          type="button"
          class="secondary-button"
          :disabled="pending || !chosen"
          @click="share"
        >
          <Link2 :size="13" aria-hidden="true" />
          Share this quantity
        </button>
      </div>
    </template>
  </section>
</template>

<style scoped>
.shared-quantity { display: grid; gap: 8px; padding: 10px; border: 1px solid var(--line); border-radius: 6px; }
.shared-quantity > header { display: grid; gap: 3px; }
.shared-quantity > header span { color: var(--muted); font-size: var(--text-sm); line-height: 1.5; }
.partner-list { display: grid; gap: 4px; margin: 0; padding: 0; list-style: none; }
.partner-list li { display: flex; align-items: center; gap: 6px; font-size: var(--text-sm); }
.form-warning { display: flex; align-items: flex-start; gap: 6px; margin: 0; color: #8a5a00; font-size: var(--text-sm); line-height: 1.5; }
.form-note { margin: 0; color: var(--muted); font-size: var(--text-sm); }
.dialog-actions button { display: inline-flex; align-items: center; gap: 6px; }
</style>
