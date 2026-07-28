<script setup lang="ts">
import { computed, ref } from 'vue'

import type { Catalogue, Distribution, Mutation, ScaleUnit, SystemModel } from '../api/types'
import { useDraft, type Draft } from '../composables/useDraft'
import { chain, nestableIn, owner } from '../domain/scaleUnits'
import type { ExpressionScope } from '../domain/squiggleLanguage'
import FieldStatus from './FieldStatus.vue'
import SquiggleEditor from './SquiggleEditor.vue'

const props = defineProps<{
  model: SystemModel
  catalogue?: Catalogue
  apply: (mutations: Mutation[]) => Promise<unknown>
}>()

/**
 * Whether the new-unit dialog is open.
 *
 * Owned by the view rather than by this panel, because the toolbar offers the
 * same gesture and the panel is not mounted while something is selected.
 */
const adding = defineModel<boolean>('adding', { default: false })

const units = computed(() => props.model.scale_units)

/**
 * What a replica count may refer to.
 *
 * Shared quantities only. A unit's size is resolved before any component is
 * solved, so a count written against a channel would name something that does
 * not exist yet.
 */
const scope = computed<ExpressionScope>(() => ({
  builtins: props.catalogue?.builtins ?? [],
  quantities: props.model.scratchpad.map((entry) => ({
    name: entry.name,
    unit: entry.unit,
    summary: entry.summary,
  })),
  locals: [],
}))

/**
 * How many times a unit's members are deployed, written out.
 *
 * The counts are expressions rather than numbers, so this multiplies them as
 * text. Reading `12 × 3` beside a unit nested in another is the whole point of
 * nesting one: the number an author has to reason about is the product, and
 * nothing else on screen says it.
 */
function deployed(unit: ScaleUnit): string {
  return chain(units.value, unit.id)
    .map((enclosing) => enclosing.replicas.trim() || '1')
    .join(' \u00d7 ')
}

/** The enclosing units whose count divides the flow rather than repeating it. */
function dividing(unit: ScaleUnit): string {
  return chain(units.value, unit.id)
    .filter((enclosing) => enclosing.distribution !== 'mirrored')
    .map((enclosing) => enclosing.replicas.trim() || '1')
    .join(' \u00d7 ')
}

const drafts = new Map<string, Draft<string>>()

function remembered(key: string, make: () => Draft<string>): Draft<string> {
  const existing = drafts.get(key)
  if (existing) return existing
  const draft = make()
  drafts.set(key, draft)
  return draft
}

function current(id: string): ScaleUnit | undefined {
  return props.model.scale_units.find((unit) => unit.id === id)
}

function fieldDraft(id: string, field: 'name' | 'replicas'): Draft<string> {
  return remembered(`${id}#${field}`, () =>
    useDraft<string>(
      () => current(id)?.[field] ?? '',
      async (value) => {
        const unit = current(id)
        if (unit) await props.apply([{ kind: 'set_scale_unit', scale_unit: { ...unit, [field]: value } }])
      },
    ),
  )
}

/**
 * What the server refused, kept against the unit it was refused for.
 *
 * The controls below save as they are changed rather than when they are left, so
 * a refusal has no field to attach itself to the way a typed expression does.
 */
const failures = ref<Record<string, string>>({})

async function amend(unit: ScaleUnit, change: Partial<ScaleUnit>) {
  try {
    await props.apply([{ kind: 'set_scale_unit', scale_unit: { ...unit, ...change } }])
    delete failures.value[unit.id]
  } catch (error) {
    failures.value = { ...failures.value, [unit.id]: (error as Error).message }
  }
}

function remove(id: string) {
  for (const key of [...drafts.keys()]) if (key.startsWith(`${id}#`)) drafts.delete(key)
  void props.apply([{ kind: 'remove_scale_unit', id }])
}

/**
 * Whether a component can join a unit, and why not where it cannot.
 *
 * A component belongs to one unit: being in two at once would give it two
 * replica counts and no way to choose between them. Saying which unit already
 * holds it turns a disabled row into an instruction.
 */
function claim(component: string, unit: string): string | null {
  const holder = owner(props.model.scale_units, component)
  return !holder || holder.id === unit ? null : holder.name || holder.id
}

// Building a new unit.
const draft = ref<ScaleUnit>(blank())
const failure = ref<string | null>(null)

function blank(): ScaleUnit {
  return {
    id: '',
    name: '',
    summary: '',
    replicas: '3',
    distribution: 'sharded',
    members: [],
    parent: null,
  }
}

const idProblem = computed(() => {
  const id = draft.value.id.trim()
  if (!id) return null
  if (units.value.some((unit) => unit.id === id)) return 'A scale unit already goes by that name.'
  if (!/^[a-z0-9][a-z0-9-]*$/.test(id)) return 'Use lower-case letters, digits and hyphens.'
  return null
})

async function add() {
  const id = draft.value.id.trim()
  if (!id || idProblem.value) return
  failure.value = null
  try {
    await props.apply([
      {
        kind: 'set_scale_unit',
        scale_unit: { ...draft.value, id, name: draft.value.name.trim() || id },
      },
    ])
    draft.value = blank()
    adding.value = false
  } catch (error) {
    failure.value = (error as Error).message
  }
}

const SPREADS: { value: Distribution; label: string; says: string }[] = [
  { value: 'sharded', label: 'Sharded', says: 'Each replica serves its share of the flow.' },
  { value: 'mirrored', label: 'Mirrored', says: 'Every replica sees the whole flow.' },
]
</script>

<template>
  <section class="scale-units">
    <header>
      <h3>Scale units</h3>
      <el-button type="primary" size="small" data-test="add-scale-unit" @click="adding = true">
        <el-icon><i-plus /></el-icon>
        <span>Add</span>
      </el-button>
    </header>

    <!--
      The empty state carries the idea, because a panel of scale units is not
      self-explanatory the way a list of quantities is. Somebody who has never
      drawn a cell boundary needs to know what one buys before they will draw
      their first.
    -->
    <p v-if="!units.length" class="blank">
      Group the components that are deployed together — a cell, a shard, a region — and say how
      many exist. Limits are then checked against one of them rather than against the fleet
      average, which is what hides a hot cell.
    </p>

    <ul class="units">
      <li v-for="unit in units" :key="unit.id" class="unit" :data-test="`scale-unit-${unit.id}`">
        <div class="head">
          <el-input
            v-model="fieldDraft(unit.id, 'name').value.value"
            size="small"
            class="name"
            :data-test="`scale-unit-name-${unit.id}`"
            @focus="fieldDraft(unit.id, 'name').focus()"
            @blur="fieldDraft(unit.id, 'name').blur()"
          />
          <FieldStatus
            :state="fieldDraft(unit.id, 'name').state.value"
            :error="fieldDraft(unit.id, 'name').error.value"
            :advice="fieldDraft(unit.id, 'name').advice.value"
            @revert="fieldDraft(unit.id, 'name').revert()"
          />
          <el-popover
            v-if="unit.summary"
            trigger="hover"
            placement="left"
            :width="300"
            :content="unit.summary"
          >
            <template #reference>
              <el-icon class="about" tabindex="0" :aria-label="`About ${unit.id}`">
                <i-info-filled />
              </el-icon>
            </template>
          </el-popover>
          <el-popconfirm
            :title="`Remove ${unit.name || unit.id}? Its members stay in the design.`"
            width="260"
            @confirm="remove(unit.id)"
          >
            <template #reference>
              <el-icon
                class="remove"
                tabindex="0"
                :aria-label="`Remove ${unit.id}`"
                :data-test="`remove-scale-unit-${unit.id}`"
              >
                <i-delete />
              </el-icon>
            </template>
          </el-popconfirm>
        </div>

        <el-alert
          v-if="failures[unit.id]"
          type="error"
          :closable="false"
          show-icon
          :title="failures[unit.id]"
        />

        <div class="field">
          <label class="label"><span class="name">how many</span></label>
          <div class="row">
            <SquiggleEditor
              v-model="fieldDraft(unit.id, 'replicas').value.value"
              :scope="scope"
              placeholder="12"
              :data-test="`scale-unit-replicas-${unit.id}`"
              @focus="fieldDraft(unit.id, 'replicas').focus()"
              @blur="fieldDraft(unit.id, 'replicas').blur()"
            />
            <FieldStatus
              :state="fieldDraft(unit.id, 'replicas').state.value"
              :error="fieldDraft(unit.id, 'replicas').error.value"
              :advice="fieldDraft(unit.id, 'replicas').advice.value"
              @revert="fieldDraft(unit.id, 'replicas').revert()"
            />
          </div>
        </div>

        <div class="pair">
          <div class="field">
            <label class="label"><span class="name">demand</span></label>
            <el-select
              :model-value="unit.distribution"
              size="small"
              :data-test="`scale-unit-distribution-${unit.id}`"
              :popper-class="`pick-distribution-${unit.id}`"
              @update:model-value="(value: unknown) => amend(unit, { distribution: value as Distribution })"
            >
              <el-option
                v-for="spread in SPREADS"
                :key="spread.value"
                :label="spread.label"
                :value="spread.value"
              >
                <span class="option">
                  <strong>{{ spread.label }}</strong>
                  <span>{{ spread.says }}</span>
                </span>
              </el-option>
            </el-select>
          </div>

          <div class="field">
            <label class="label"><span class="name">inside</span></label>
            <el-select
              :model-value="unit.parent ?? ''"
              size="small"
              clearable
              placeholder="nothing"
              :data-test="`scale-unit-parent-${unit.id}`"
              :popper-class="`pick-parent-${unit.id}`"
              @update:model-value="(value: unknown) => amend(unit, { parent: (value as string) || null })"
            >
              <el-option
                v-for="candidate in nestableIn(units, unit.id)"
                :key="candidate.id"
                :label="candidate.name || candidate.id"
                :value="candidate.id"
              />
            </el-select>
          </div>
        </div>

        <div class="field">
          <label class="label"><span class="name">holds</span></label>
          <el-select
            :model-value="unit.members"
            multiple
            filterable
            size="small"
            placeholder="No components yet"
            :data-test="`scale-unit-members-${unit.id}`"
            :popper-class="`pick-members-${unit.id}`"
            @update:model-value="(value: unknown) => amend(unit, { members: value as string[] })"
          >
            <el-option
              v-for="component in model.components"
              :key="component.id"
              :label="component.name || component.id"
              :value="component.id"
              :disabled="!!claim(component.id, unit.id)"
            >
              <span class="option">
                <strong>{{ component.name || component.id }}</strong>
                <span v-if="claim(component.id, unit.id)">
                  already in {{ claim(component.id, unit.id) }}
                </span>
                <span v-else>{{ component.type }}</span>
              </span>
            </el-option>
          </el-select>
        </div>

        <p class="tally" :data-test="`scale-unit-tally-${unit.id}`">
          Deployed <code>{{ deployed(unit) }}</code> times<template v-if="dividing(unit)">, each
          serving <code>1 / ({{ dividing(unit) }})</code> of the flow</template
          >.
        </p>
      </li>
    </ul>

    <el-dialog v-model="adding" title="New scale unit" width="520px">
      <el-form label-position="top" size="small" @submit.prevent="add">
        <el-form-item label="Identifier" :error="idProblem ?? undefined">
          <el-input v-model="draft.id" placeholder="cell" data-test="new-scale-unit-id" />
        </el-form-item>
        <el-form-item label="Name">
          <el-input v-model="draft.name" placeholder="Serving cell" data-test="new-scale-unit-name" />
        </el-form-item>
        <el-form-item label="How many of them exist">
          <SquiggleEditor
            v-model="draft.replicas"
            :scope="scope"
            placeholder="12"
            data-test="new-scale-unit-replicas"
          />
        </el-form-item>

        <!--
          Asked here rather than left at a default, because the two answers size
          a design differently and the wrong one is invisible afterwards. A
          mirrored unit that was assumed sharded is a system built for a twelfth
          of the load it will actually see.
        -->
        <el-form-item label="How demand meets them">
          <el-radio-group v-model="draft.distribution" data-test="new-scale-unit-distribution">
            <el-radio v-for="spread in SPREADS" :key="spread.value" :value="spread.value">
              {{ spread.label }} &mdash; {{ spread.says.toLowerCase().replace(/\.$/, '') }}
            </el-radio>
          </el-radio-group>
        </el-form-item>

        <el-form-item label="Components inside">
          <el-select
            v-model="draft.members"
            multiple
            filterable
            placeholder="Choose the components deployed together"
            data-test="new-scale-unit-members"
            popper-class="pick-new-members"
          >
            <el-option
              v-for="component in model.components"
              :key="component.id"
              :label="component.name || component.id"
              :value="component.id"
              :disabled="!!owner(units, component.id)"
            />
          </el-select>
        </el-form-item>

        <el-form-item v-if="units.length" label="Nested inside">
          <el-select
            v-model="draft.parent"
            clearable
            placeholder="nothing"
            data-test="new-scale-unit-parent"
            popper-class="pick-new-parent"
          >
            <el-option
              v-for="candidate in units"
              :key="candidate.id"
              :label="candidate.name || candidate.id"
              :value="candidate.id"
            />
          </el-select>
        </el-form-item>

        <el-form-item label="What this boundary is">
          <el-input v-model="draft.summary" type="textarea" :rows="2" />
        </el-form-item>
        <el-alert v-if="failure" type="error" :closable="false" show-icon :title="failure" />
      </el-form>
      <template #footer>
        <el-button size="small" @click="adding = false">Cancel</el-button>
        <el-button
          type="primary"
          size="small"
          :disabled="!draft.id.trim() || !!idProblem"
          data-test="save-scale-unit"
          @click="add"
        >
          Add
        </el-button>
      </template>
    </el-dialog>
  </section>
</template>

<style scoped>
.scale-units { display: flex; flex-direction: column; gap: var(--space-3); }
header { display: flex; align-items: center; justify-content: space-between; gap: var(--space-4); }
h3 { font-size: var(--text-md); margin: 0; }
.blank { margin: 0; font-size: var(--text-xs); line-height: 1.5; color: var(--muted); }
.units { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: var(--space-3); }
.unit {
  padding: var(--space-3);
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface);
}
.head { display: flex; align-items: center; gap: var(--space-2); margin-bottom: var(--space-2); }
.head .name { flex: 1; min-width: 0; }
.about, .remove { font-size: 13px; color: var(--muted); cursor: pointer; }
.about:hover { color: var(--green); }
.remove:hover { color: var(--danger); }
.field { margin-bottom: var(--space-2); }
.field :deep(.el-select) { width: 100%; }
.label { display: flex; align-items: center; gap: var(--space-1); margin-bottom: 3px; }
.label .name { font-family: var(--mono); font-size: var(--text-2xs); color: var(--muted); }
.row { display: flex; align-items: flex-start; gap: var(--space-2); }
.row > :first-child { flex: 1; min-width: 0; }
.pair { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-2); }
.option { display: flex; flex-direction: column; line-height: 1.3; padding: 3px 0; }
.option span { font-size: var(--text-2xs); color: var(--muted); }
.tally { margin: var(--space-2) 0 0; font-size: var(--text-2xs); color: var(--muted); line-height: 1.5; }
.tally code { font-family: var(--mono); overflow-wrap: anywhere; }
.unit :deep(.el-alert) { margin-bottom: var(--space-2); }
</style>
