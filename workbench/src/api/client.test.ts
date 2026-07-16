import { afterEach, describe, expect, it, vi } from 'vitest'
import { api, OptimistApiError } from './client'
import type { Project, ProjectArchive } from './types'

const project: Project = { id: 'A', name: 'Delivery', revision: 7 }

afterEach(() => vi.unstubAllGlobals())

describe('Optimist API client', () => {
  it('sends revision-checked idempotent node commands', async () => {
    const node = {
      id: 'B',
      revision: 0,
      name: 'feedback',
      normalized_name: 'feedback',
      title: 'Fast feedback',
      description: '',
      aliases: [],
      metadata: {},
      payload: {
        kind: 'factor' as const,
        properties: { current: null, desired: null, controllable: true, evidence: [] },
      },
    }
    const fetch = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          request_id: '00000000-0000-4000-8000-000000000000',
          project_revision: 8,
          outcome: { type: 'node_created', value: node },
        }),
        { status: 201, headers: { 'Content-Type': 'application/json' } },
      ),
    )
    vi.stubGlobal('fetch', fetch)
    vi.stubGlobal('crypto', {
      randomUUID: () => '00000000-0000-4000-8000-000000000000',
    })

    await expect(
      api.createNode(project, {
        name: 'feedback',
        title: 'Fast feedback',
        payload: node.payload,
      }),
    ).resolves.toEqual(node)

    const [url, request] = fetch.mock.calls[0] as [string, RequestInit]
    expect(url).toBe('/api/v1/projects/A/commands')
    expect(JSON.parse(request.body as string)).toEqual({
      request_id: '00000000-0000-4000-8000-000000000000',
      expected_revision: 7,
      command: {
        type: 'create_node',
        payload: { name: 'feedback', title: 'Fast feedback', payload: node.payload },
      },
    })
  })

  it('preserves stable API errors and recovery advice', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(
        new Response(
          JSON.stringify({
            error: {
              code: 'project_revision_conflict',
              message: 'project revision conflict',
              advice: ['Refresh the project.'],
            },
          }),
          { status: 409, headers: { 'Content-Type': 'application/json' } },
        ),
      ),
    )

    const error = await api.project('A').catch((value: unknown) => value)
    expect(error).toBeInstanceOf(OptimistApiError)
    expect(error).toMatchObject({
      code: 'project_revision_conflict',
      message: 'project revision conflict',
      advice: ['Refresh the project.'],
    })
  })

  it('sends typed structural relationship commands', async () => {
    const edge = {
      source: 'A',
      source_kind: 'factor',
      destination: 'B',
      destination_kind: 'factor',
      revision: 0,
      description: '',
      metadata: {},
      payload: { kind: 'part_of' },
    }
    const fetch = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          request_id: '00000000-0000-4000-8000-000000000000',
          project_revision: 8,
          outcome: { type: 'edge_created', value: edge },
        }),
        { status: 201, headers: { 'Content-Type': 'application/json' } },
      ),
    )
    vi.stubGlobal('fetch', fetch)
    vi.stubGlobal('crypto', {
      randomUUID: () => '00000000-0000-4000-8000-000000000000',
    })

    await expect(
      api.createEdge(project, {
        source: 'A',
        destination: 'B',
        payload: { kind: 'part_of' },
      }),
    ).resolves.toEqual(edge)
    expect(JSON.parse((fetch.mock.calls[0]![1] as RequestInit).body as string)).toMatchObject({
      expected_revision: 7,
      command: {
        type: 'create_edge',
        payload: { source: 'A', destination: 'B', payload: { kind: 'part_of' } },
      },
    })
  })

  it('exports and explicitly replaces portable project archives', async () => {
    const archive: ProjectArchive = {
      schema_version: 1,
      project,
      files: { '_project.md': '---\n---\n' },
      summary: { entities: 0, edges: 0, scenarios: 0 },
    }
    const fetch = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(JSON.stringify(archive), {
          status: 200,
          headers: { 'Content-Type': 'application/json' },
        }),
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify(project), {
          status: 201,
          headers: { 'Content-Type': 'application/json' },
        }),
      )
    vi.stubGlobal('fetch', fetch)

    await expect(api.exportProject('A')).resolves.toEqual(archive)
    await expect(api.importProject(archive, true)).resolves.toEqual(project)
    expect(fetch.mock.calls[0]![0]).toBe('/api/v1/projects/A/archive')
    expect(fetch.mock.calls[1]![0]).toBe('/api/v1/project-archives?replace=true&yes=true')
    expect(JSON.parse((fetch.mock.calls[1]![1] as RequestInit).body as string)).toEqual(archive)
  })

  it('sends aggregate-revision checked node metadata updates', async () => {
    const node = {
      id: 'A',
      revision: 3,
      name: 'feedback',
      normalized_name: 'feedback',
      title: 'Fast feedback',
      description: '',
      aliases: [],
      metadata: {},
      payload: {
        kind: 'factor' as const,
        properties: { current: null, desired: null, controllable: true, evidence: [] },
      },
    }
    const updated = { ...node, revision: 4, title: 'Rapid feedback' }
    const fetch = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          request_id: '00000000-0000-4000-8000-000000000000',
          project_revision: 8,
          outcome: { type: 'node_metadata_updated', value: updated },
        }),
        { status: 201, headers: { 'Content-Type': 'application/json' } },
      ),
    )
    vi.stubGlobal('fetch', fetch)
    vi.stubGlobal('crypto', {
      randomUUID: () => '00000000-0000-4000-8000-000000000000',
    })

    await api.updateNode(project, node, {
      title: 'Rapid feedback',
      description: 'Short feedback loops.',
      metadata: { owner: 'platform' },
    })
    expect(JSON.parse((fetch.mock.calls[0]![1] as RequestInit).body as string)).toMatchObject({
      expected_revision: 7,
      command: {
        type: 'update_node_metadata',
        payload: {
          id: 'A',
          expected_revision: 3,
          title: 'Rapid feedback',
          description: 'Short feedback loops.',
          metadata: { owner: 'platform' },
        },
      },
    })
  })

  it('allocates distinct current and desired state estimate addresses', async () => {
    const node = {
      id: 'A',
      revision: 0,
      name: 'feedback',
      normalized_name: 'feedback',
      title: 'Fast feedback',
      description: '',
      aliases: [],
      metadata: {},
      payload: {
        kind: 'factor' as const,
        properties: {
          current: {
            id: 'A',
            revision: 0,
            distribution: { type: 'point' as const, value: 0.4 },
          },
          desired: null,
          controllable: true,
          evidence: [],
        },
      },
    }
    const fetch = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          request_id: '00000000-0000-4000-8000-000000000000',
          project_revision: 8,
          outcome: {
            type: 'estimate_set',
            value: {
              address: { project: 'A', owner: { kind: 'node', id: 'A' }, estimate: 'B' },
              slot: { kind: 'desired' },
              revision: 0,
              distribution: { type: 'beta', alpha: 8, beta: 2 },
              provenance: ['planning'],
            },
          },
        }),
        { status: 201, headers: { 'Content-Type': 'application/json' } },
      ),
    )
    vi.stubGlobal('fetch', fetch)
    vi.stubGlobal('crypto', {
      randomUUID: () => '00000000-0000-4000-8000-000000000000',
    })

    await api.setStateEstimate(project, node, {
      slot: 'desired',
      distribution: { type: 'beta', alpha: 8, beta: 2 },
      provenance: ['planning'],
    })
    expect(JSON.parse((fetch.mock.calls[0]![1] as RequestInit).body as string)).toMatchObject({
      command: {
        type: 'set_estimate',
        payload: {
          address: { project: 'A', owner: { kind: 'node', id: 'A' }, estimate: 'B' },
          slot: { kind: 'desired' },
          distribution: { type: 'beta', alpha: 8, beta: 2 },
          provenance: ['planning'],
        },
      },
    })
  })
})
