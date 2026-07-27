import type { Mutation, SystemModel } from '../api/types'

/**
 * Applies one edit to a local copy of a design.
 *
 * This mirrors the server's own application of the same message. Both exist
 * because the feed carries edits rather than snapshots: replaying an edit here
 * leaves every other part of the design alone, including the field somebody is
 * typing into, which replacing the design wholesale would not.
 *
 * Each edit names one entity and replaces it entirely, so applying the same
 * message twice leaves the same result. That is what makes a reconnect safe
 * without tracking which messages were already seen.
 */
export function applyMutation(model: SystemModel, mutation: Mutation): SystemModel {
  switch (mutation.kind) {
    case 'set_scratchpad_entry':
      return { ...model, scratchpad: replace(model.scratchpad, mutation.entry, 'name') }
    case 'remove_scratchpad_entry':
      return { ...model, scratchpad: model.scratchpad.filter((e) => e.name !== mutation.name) }
    case 'set_component':
      return { ...model, components: replace(model.components, mutation.component, 'id') }
    case 'remove_component':
      return {
        ...model,
        components: model.components.filter((c) => c.id !== mutation.id),
        // A relationship to a component that no longer exists cannot be solved,
        // so the server drops these too. Keeping them locally would show an edge
        // that vanishes on the next reload.
        relationships: model.relationships.filter(
          (r) => r.from !== mutation.id && r.to !== mutation.id,
        ),
      }
    case 'set_relationship': {
      const rest = model.relationships.filter(
        (r) => !(r.from === mutation.relationship.from && r.to === mutation.relationship.to),
      )
      return { ...model, relationships: [...rest, mutation.relationship] }
    }
    case 'remove_relationship':
      return {
        ...model,
        relationships: model.relationships.filter(
          (r) => !(r.from === mutation.from && r.to === mutation.to),
        ),
      }
    case 'set_scale_unit':
      return { ...model, scale_units: replace(model.scale_units, mutation.scale_unit, 'id') }
    case 'remove_scale_unit':
      return { ...model, scale_units: model.scale_units.filter((u) => u.id !== mutation.id) }
    case 'set_intervention':
      return { ...model, interventions: replace(model.interventions, mutation.intervention, 'id') }
    case 'remove_intervention':
      return { ...model, interventions: model.interventions.filter((i) => i.id !== mutation.id) }
  }
}

/**
 * Replaces an entry in place, or appends it where it is new.
 *
 * Position is preserved on replacement so that editing something does not make
 * it jump to the end of the list a reader is looking at.
 */
function replace<T, K extends keyof T>(items: T[], next: T, key: K): T[] {
  const index = items.findIndex((item) => item[key] === next[key])
  if (index < 0) return [...items, next]
  const copy = items.slice()
  copy[index] = next
  return copy
}
