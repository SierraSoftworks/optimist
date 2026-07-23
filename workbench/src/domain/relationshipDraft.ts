import type { EdgeKind } from '../api/types'

const key = 'optimist.relationshipDraft'
const kinds = new Set<EdgeKind>([
  'contributes', 'measures', 'changes', 'requires', 'part_of', 'blocks',
  'conflicts_with', 'synergizes_with',
])

export interface RelationshipDraft {
  projectId: string
  sourceId: string
  destinationId: string
  kind: EdgeKind
  sourceLocked: boolean
}

export function readRelationshipDraft(storage: Pick<Storage, 'getItem'>) {
  try {
    const value = JSON.parse(storage.getItem(key) ?? 'null') as Partial<RelationshipDraft> | null
    if (
      !value || typeof value.projectId !== 'string' || typeof value.sourceId !== 'string' ||
      typeof value.destinationId !== 'string' || typeof value.kind !== 'string' ||
      !kinds.has(value.kind as EdgeKind) || typeof value.sourceLocked !== 'boolean'
    ) return null
    return value as RelationshipDraft
  } catch {
    return null
  }
}

export function writeRelationshipDraft(
  storage: Pick<Storage, 'setItem' | 'removeItem'>,
  draft: RelationshipDraft | null,
) {
  if (draft) storage.setItem(key, JSON.stringify(draft))
  else storage.removeItem(key)
}