import { describe, expect, it } from 'vitest'

import type { Component, Mutation, SystemModel } from '../api/types'
import { applyMutation } from './applyMutation'

function model(): SystemModel {
  return {
    scratchpad: [
      { name: 'rate', expression: '100', unit: 'op/s', summary: 'Demand.' },
      { name: 'size', expression: '8', unit: 'op', summary: 'Pool.' },
    ],
    components: [
      { id: 'users', name: 'Users', type: 'client', properties: { request_rate: 'rate' } },
      { id: 'api', name: 'API', type: 'compute', properties: { parallelism: 'size' } },
    ],
    relationships: [{ from: 'users', to: 'api', summary: 'Requests.', mutators: [] }],
    scale_units: [],
    interventions: [
      { id: 'quieter', name: 'Quieter', summary: '', overrides: [{ name: 'rate', expression: '10' }] },
    ],
  }
}

function apply(start: SystemModel, ...mutations: Mutation[]): SystemModel {
  return mutations.reduce(applyMutation, start)
}

describe('applyMutation', () => {
  it('replaces a shared quantity without moving it', () => {
    const next = apply(model(), {
      kind: 'set_scratchpad_entry',
      entry: { name: 'rate', expression: '250', unit: 'op/s', summary: 'Demand.' },
    })
    expect(next.scratchpad.map((entry) => entry.name)).toEqual(['rate', 'size'])
    expect(next.scratchpad[0].expression).toBe('250')
  })

  it('appends a quantity it has not seen', () => {
    const next = apply(model(), {
      kind: 'set_scratchpad_entry',
      entry: { name: 'depth', expression: '8', unit: '1', summary: '' },
    })
    expect(next.scratchpad).toHaveLength(3)
    expect(next.scratchpad[2].name).toBe('depth')
  })

  it('moves a shared quantity before another one idempotently', () => {
    const edit: Mutation = { kind: 'move_scratchpad_entry', name: 'size', before: 'rate' }

    const once = apply(model(), edit)
    const twice = apply(once, edit)

    expect(once.scratchpad.map((entry) => entry.name)).toEqual(['size', 'rate'])
    expect(twice).toEqual(once)
  })

  /**
   * The feed replays edits, and a reconnect can deliver one that was already
   * seen. Applying it twice has to leave the same design or the local copy
   * drifts from the server's without anything noticing.
   */
  it('is idempotent', () => {
    const edit: Mutation = {
      kind: 'set_component',
      component: { id: 'api', name: 'API', type: 'compute', properties: { parallelism: '32' } },
    }
    const once = apply(model(), edit)
    const twice = apply(model(), edit, edit)
    expect(twice).toEqual(once)
  })

  it('leaves everything it does not name alone', () => {
    const start = model()
    const next = apply(start, { kind: 'remove_scratchpad_entry', name: 'size' })
    expect(next.components).toBe(start.components)
    expect(next.relationships).toBe(start.relationships)
    expect(next.scratchpad.map((entry) => entry.name)).toEqual(['rate'])
  })

  /**
   * A relationship to a component that is gone cannot be solved, and the server
   * drops it. Keeping it locally would draw an edge that disappears on reload.
   */
  it('drops the relationships of a removed component', () => {
    const next = apply(model(), { kind: 'remove_component', id: 'api' })
    expect(next.components.map((component) => component.id)).toEqual(['users'])
    expect(next.relationships).toHaveLength(0)
  })

  it('keys a relationship on both of its ends', () => {
    const start = apply(model(), {
      kind: 'set_relationship',
      relationship: { from: 'api', to: 'users', summary: 'Responses.', mutators: [] },
    })
    expect(start.relationships).toHaveLength(2)

    const next = apply(start, { kind: 'remove_relationship', from: 'users', to: 'api' })
    expect(next.relationships).toHaveLength(1)
    expect(next.relationships[0].from).toBe('api')
  })

  it('replaces a relationship rather than duplicating it', () => {
    const next = apply(model(), {
      kind: 'set_relationship',
      relationship: {
        from: 'users',
        to: 'api',
        summary: 'Requests.',
        mutators: [{ type: 'retry', properties: { attempts: '3' } }],
      },
    })
    expect(next.relationships).toHaveLength(1)
    expect(next.relationships[0].mutators).toHaveLength(1)
  })

  /**
   * The server drops a removed component from every unit that held it, because
   * a unit naming a component that is gone will not compile. Leaving it here
   * would show a member that vanishes on the next reload.
   */
  it('releases a removed component from its scale unit', () => {
    const grouped = apply(model(), {
      kind: 'set_scale_unit',
      scale_unit: {
        id: 'cell',
        name: 'Cell',
        summary: '',
        replicas: '12',
        distribution: 'sharded',
        members: ['users', 'api'],
      },
    })
    const next = apply(grouped, { kind: 'remove_component', id: 'api' })
    expect(next.scale_units[0].members).toEqual(['users'])
  })

  /**
   * Nesting names an enclosing unit, so removing one has to release whatever
   * sat inside it. A parent that resolves to nothing gives its members no
   * replica count at all.
   */
  it('lifts a nested unit out when its enclosure is removed', () => {
    const unit = (id: string, parent?: string) => ({
      kind: 'set_scale_unit' as const,
      scale_unit: {
        id,
        name: id,
        summary: '',
        replicas: '3',
        distribution: 'sharded' as const,
        members: [],
        parent: parent ?? null,
      },
    })
    const nested = apply(model(), unit('region'), unit('cell', 'region'))
    const next = apply(nested, { kind: 'remove_scale_unit', id: 'region' })
    expect(next.scale_units.map((entry) => entry.id)).toEqual(['cell'])
    expect(next.scale_units[0].parent).toBeNull()
  })

  it('sets and removes interventions', () => {
    const added = apply(model(), {
      kind: 'set_intervention',
      intervention: { id: 'louder', name: 'Louder', summary: '', overrides: [] },
    })
    expect(added.interventions.map((entry) => entry.id)).toEqual(['quieter', 'louder'])

    const removed = apply(added, { kind: 'remove_intervention', id: 'quieter' })
    expect(removed.interventions.map((entry) => entry.id)).toEqual(['louder'])
  })

  it('does not mutate the design it was given', () => {
    const start = model()
    const before = JSON.stringify(start)
    apply(start, { kind: 'remove_component', id: 'api' })
    apply(start, {
      kind: 'set_component',
      component: { id: 'api', name: 'Renamed', type: 'compute', properties: {} } as Component,
    })
    expect(JSON.stringify(start)).toBe(before)
  })
})
