import { describe, expect, it } from 'vitest'

import type { ScaleUnit } from '../api/types'
import { chain, inhabited, nestableIn, owner } from './scaleUnits'

function unit(id: string, parent: string | null, members: string[] = []): ScaleUnit {
  return { id, name: id, summary: '', replicas: '2', distribution: 'sharded', members, parent }
}

/**
 * Written parents-before-children in one direction and the reverse in another,
 * because the model preserves author order and an editor cannot assume a list
 * arrives sorted by depth.
 */
function fleet(): ScaleUnit[] {
  return [unit('shard', 'cell', ['store']), unit('region', null), unit('cell', 'region', ['api'])]
}

describe('chain', () => {
  it('reads from the unit outward', () => {
    expect(chain(fleet(), 'shard').map((entry) => entry.id)).toEqual(['shard', 'cell', 'region'])
  })

  it('ends at a unit that encloses nothing', () => {
    expect(chain(fleet(), 'region').map((entry) => entry.id)).toEqual(['region'])
  })

  /**
   * A loop cannot be saved, but it can be held on screen while somebody is
   * part-way through rearranging one. Walking forever is the only outcome that
   * is worse than returning what was walked.
   */
  it('stops where a loop closes', () => {
    const looped = [unit('a', 'b'), unit('b', 'a')]
    expect(chain(looped, 'a').map((entry) => entry.id)).toEqual(['a', 'b'])
  })

  it('has nothing to say about a unit that does not exist', () => {
    expect(chain(fleet(), 'absent')).toEqual([])
  })
})

describe('nestableIn', () => {
  it('offers neither the unit itself nor anything already within it', () => {
    expect(nestableIn(fleet(), 'cell').map((entry) => entry.id)).toEqual(['region'])
  })

  it('offers everything else to a unit nothing is inside', () => {
    expect(nestableIn(fleet(), 'shard').map((entry) => entry.id)).toEqual(['region', 'cell'])
  })
})

describe('owner', () => {
  it('names the unit holding a component', () => {
    expect(owner(fleet(), 'api')?.id).toBe('cell')
  })

  it('says nothing about a component in no unit', () => {
    expect(owner(fleet(), 'users')).toBeUndefined()
  })
})

describe('inhabited', () => {
  /**
   * A unit whose only content is another unit still bounds something, so it is
   * still worth drawing. Judging on direct members alone would put a shard on
   * the diagram with no cell around it.
   */
  it('counts a unit that only holds other units', () => {
    expect([...inhabited(fleet())].sort()).toEqual(['cell', 'region', 'shard'])
  })

  it('leaves out a unit with nothing anywhere inside it', () => {
    expect(inhabited([unit('spare', null)]).size).toBe(0)
  })
})
