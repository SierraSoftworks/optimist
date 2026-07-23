import { describe, expect, it } from 'vitest'
import { readRelationshipDraft, writeRelationshipDraft } from './relationshipDraft'

describe('relationship draft persistence', () => {
  it('round-trips selected endpoints in tab-scoped storage', () => {
    const values = new Map<string, string>()
    const storage = {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => values.set(key, value),
      removeItem: (key: string) => values.delete(key),
    }
    const draft = {
      projectId: 'A', sourceId: 'B', destinationId: 'C',
      kind: 'contributes' as const, sourceLocked: false,
    }
    writeRelationshipDraft(storage, draft)
    expect(readRelationshipDraft(storage)).toEqual(draft)
    writeRelationshipDraft(storage, null)
    expect(readRelationshipDraft(storage)).toBeNull()
  })

  it('rejects malformed or unknown relationship drafts', () => {
    expect(readRelationshipDraft({ getItem: () => '{' })).toBeNull()
    expect(readRelationshipDraft({
      getItem: () => JSON.stringify({
        projectId: 'A', sourceId: 'B', destinationId: 'C',
        kind: 'unknown', sourceLocked: false,
      }),
    })).toBeNull()
  })
})