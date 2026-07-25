import type {
  EstimateAddress,
  ProjectDependenceModel,
  ResidualDependenceGroup,
} from '../api/types'

/**
 * Editing rules for the shared-quantity idiom over residual dependence groups.
 *
 * Two estimates that stand for the same real quantity are a dependence problem
 * rather than a duplication problem: give them the same marginal and couple them
 * at a correlation of one, and every draw hands both the same value. That needs
 * no reference from one estimate's source to the other, so an estimate stays
 * independently parseable.
 *
 * A group whose matrix is entirely ones is a shared quantity. Any other matrix
 * is a hand-authored correlation, and these helpers refuse to rewrite one rather
 * than silently reinterpreting a modeller's coefficients.
 */

export function sameAddress(left: EstimateAddress, right: EstimateAddress): boolean {
  if (left.project !== right.project || left.estimate !== right.estimate) return false
  if (left.owner.kind !== right.owner.kind) return false
  if (left.owner.kind === 'node') return left.owner.id === right.owner.id
  const other = right.owner.id as { source: string; kind: string; destination: string }
  return (
    left.owner.id.source === other.source &&
    left.owner.id.kind === other.kind &&
    left.owner.id.destination === other.destination
  )
}

/** Returns the group containing `address`, or `null` when it is uncoupled. */
export function groupOf(
  model: ProjectDependenceModel | null,
  address: EstimateAddress,
): ResidualDependenceGroup | null {
  return (
    model?.residual_groups.find((group) =>
      group.members.some((member) => sameAddress(member, address)),
    ) ?? null
  )
}

/** Returns every estimate coupled with `address`, excluding it. */
export function partnersOf(
  model: ProjectDependenceModel | null,
  address: EstimateAddress,
): EstimateAddress[] {
  const group = groupOf(model, address)
  if (!group) return []
  return group.members.filter((member) => !sameAddress(member, address))
}

/** Reports whether a group couples its members as one shared quantity. */
export function isSharedQuantity(group: ResidualDependenceGroup): boolean {
  return group.correlation.matrix.every((row) => row.every((value) => value === 1))
}

export class CouplingConflict extends Error {}

/**
 * Couples `address` to `partner` as one shared quantity.
 *
 * Groups may not overlap, so an estimate already inside a hand-authored
 * correlation cannot join a shared quantity without discarding coefficients
 * somebody chose. That case throws instead, leaving the decision with the author.
 */
export function shareQuantity(
  model: ProjectDependenceModel | null,
  address: EstimateAddress,
  partner: EstimateAddress,
): ProjectDependenceModel {
  const current = model ?? { revision: 0, residual_groups: [] }
  const existing = groupOf(current, address)
  const partnerGroup = groupOf(current, partner)
  if (existing && partnerGroup && existing === partnerGroup) return current
  for (const group of [existing, partnerGroup]) {
    if (group && !isSharedQuantity(group)) {
      throw new CouplingConflict(
        'One of these estimates already carries an authored correlation. Remove it before sharing a quantity.',
      )
    }
  }
  if (existing && partnerGroup) {
    throw new CouplingConflict(
      'Both estimates already share a quantity with others. Stop sharing one of them first.',
    )
  }
  const group = existing ?? partnerGroup
  const members = group
    ? [...group.members, group === existing ? partner : address]
    : [address, partner]
  return replaceGroup(current, group, shared(members))
}

/**
 * Removes `address` from its shared quantity, keeping the rest coupled.
 *
 * A group needs at least two members, so removing the second-to-last member
 * drops the group entirely rather than leaving an invalid document behind.
 */
export function stopSharing(
  model: ProjectDependenceModel,
  address: EstimateAddress,
): ProjectDependenceModel {
  const group = groupOf(model, address)
  if (!group) return model
  const members = group.members.filter((member) => !sameAddress(member, address))
  return replaceGroup(model, group, members.length < 2 ? null : shared(members))
}

function shared(members: EstimateAddress[]): ResidualDependenceGroup {
  return {
    members,
    correlation: {
      scale: 'latent',
      matrix: members.map(() => members.map(() => 1)),
    },
  }
}

function replaceGroup(
  model: ProjectDependenceModel,
  previous: ResidualDependenceGroup | null,
  next: ResidualDependenceGroup | null,
): ProjectDependenceModel {
  const groups = model.residual_groups.filter((group) => group !== previous)
  return {
    revision: model.revision,
    residual_groups: next ? [...groups, next] : groups,
  }
}
