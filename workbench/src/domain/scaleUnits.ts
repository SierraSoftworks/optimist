import type { ScaleUnit } from '../api/types'

/**
 * Reading the nesting of scale units.
 *
 * A unit names a boundary that is replicated as a whole — a cell, a shard, a
 * region — and units nest, because deployments do. The server rejects a nesting
 * that closes into a loop and a component claimed by two units at once; the
 * questions here are the ones the editor has to answer *before* it offers a
 * choice, so that an impossible arrangement is never on the menu.
 */

/**
 * The units enclosing one, innermost first, starting with the unit itself.
 *
 * The order is the order in which replica counts multiply, which is what a
 * reader is being told when the chain is printed: a shard inside a region is
 * deployed once per region per shard.
 *
 * A chain that closes into a loop stops where it began. That arrangement cannot
 * be saved, but it can be *arrived at* while an author is part-way through
 * rearranging one, and returning a partial chain is better than not returning.
 */
export function chain(units: ScaleUnit[], id: string): ScaleUnit[] {
  const byId = new Map(units.map((unit) => [unit.id, unit]))
  const walked: ScaleUnit[] = []
  const seen = new Set<string>()
  let current = byId.get(id)
  while (current && !seen.has(current.id)) {
    seen.add(current.id)
    walked.push(current)
    current = current.parent ? byId.get(current.parent) : undefined
  }
  return walked
}

/**
 * The units one may be nested inside.
 *
 * Everything except itself and whatever already sits inside it. Nesting a unit
 * within its own descendant is the one arrangement that has no outermost level,
 * so no component in it would have a replica count at all.
 */
export function nestableIn(units: ScaleUnit[], id: string): ScaleUnit[] {
  const inside = new Set([id])
  // Repeated until nothing new is claimed, because the list is in author order
  // rather than parents-before-children.
  for (let pass = 0; pass < units.length; pass += 1) {
    const before = inside.size
    for (const unit of units) {
      if (unit.parent && inside.has(unit.parent)) inside.add(unit.id)
    }
    if (inside.size === before) break
  }
  return units.filter((unit) => !inside.has(unit.id))
}

/** The unit a component is deployed in, of which there is at most one. */
export function owner(units: ScaleUnit[], component: string): ScaleUnit | undefined {
  return units.find((unit) => unit.members.includes(component))
}

/**
 * The units with something in them, directly or further in.
 *
 * An empty unit has nothing to draw a boundary around, and a diagram that drew
 * one anyway would show a box that no component is inside and that no amount of
 * rearranging makes meaningful.
 */
export function inhabited(units: ScaleUnit[]): Set<string> {
  const holding = new Set(units.filter((unit) => unit.members.length).map((unit) => unit.id))
  for (let pass = 0; pass < units.length; pass += 1) {
    const before = holding.size
    for (const unit of units) {
      if (unit.parent && holding.has(unit.id)) holding.add(unit.parent)
    }
    if (holding.size === before) break
  }
  return holding
}
