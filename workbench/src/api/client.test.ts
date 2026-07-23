import { afterEach, describe, expect, it, vi } from 'vitest'
import { api, OptimistApiError } from './client'
import type { Project, ProjectArchive } from './types'

const project: Project = { id: 'A', name: 'Delivery', revision: 7 }

afterEach(() => vi.unstubAllGlobals())

describe('Optimist API client', () => {
  it('assesses a unit-aware Fermi decomposition', async () => {
    const assessment = {
      compiled: { unit: {}, dependencies: [] },
      report: {
        estimates: [{ mean: 0.6, variance: 0.04, mean_standard_error: 0.001, variance_standard_error: 0.002 }],
        covariance: [[0.04]],
        diagnostics: {
          seed: 42, attempted_samples: 1000, valid_samples: 1000,
          invalid_samples: { zero_denominator: 0, non_finite_primitive: 0, non_finite_result: 0 },
          criterion: { seed: 42, minimum_samples: 1000, maximum_samples: 10000, absolute_tolerance: 0.001, relative_tolerance: 0.01 },
          status: 'converged',
        },
      },
      recommendation: { status: 'moment_matched', distribution: { type: 'beta', alpha: 3, beta: 2 }, interval: { probability: 0.9, lower: 0.2, upper: 0.9 }, warning: 'Approximation' },
    }
    const fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(assessment), {
        status: 200, headers: { 'Content-Type': 'application/json' },
      }),
    )
    vi.stubGlobal('fetch', fetch)
    const formula = {
      type: 'product' as const,
      factors: [
        { type: 'literal' as const, distribution: { type: 'point' as const, value: 0.6 }, unit: {} },
        { type: 'literal' as const, distribution: { type: 'point' as const, value: 1 }, unit: {} },
      ],
    }
    await expect(api.assessFermi('A', {
      formula,
      support: 'probability',
      expected_unit: {},
      monte_carlo: assessment.report.diagnostics.criterion,
    })).resolves.toEqual(assessment)
    expect(fetch.mock.calls[0]![0]).toBe('/api/v1/projects/A/analysis/fermi-assessment')
    expect(JSON.parse(fetch.mock.calls[0]![1].body)).toMatchObject({
      support: 'probability', expected_unit: {}, formula: { type: 'product' },
    })
  })

  it('sets measurement calibration under project and edge revision guards', async () => {
    const edge = {
      source: 'A', source_kind: 'metric' as const,
      destination: 'B', destination_kind: 'factor' as const,
      revision: 3, description: '', metadata: {},
      payload: { kind: 'measures' as const, properties: { polarity: 'lower_is_better' as const, observations: [] } },
    }
    const fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({
        request_id: 'request', project_revision: 8,
        outcome: {
          type: 'measurement_calibration_set',
          value: { ...edge, revision: 4, payload: { ...edge.payload, properties: { ...edge.payload.properties, calibration: { type: 'linear', state_zero: 20, state_one: 5 } } } },
        },
      }), { status: 201, headers: { 'Content-Type': 'application/json' } }),
    )
    vi.stubGlobal('fetch', fetch)
    await api.setMeasurementCalibration(project, edge, {
      calibration: { type: 'linear', state_zero: 20, state_one: 5 },
    })
    expect(JSON.parse(fetch.mock.calls[0]![1].body)).toMatchObject({
      expected_revision: 7,
      command: {
        type: 'set_measurement_calibration',
        payload: {
          edge: { source: 'A', kind: 'measures', destination: 'B' },
          expected_revision: 3,
          calibration: { type: 'linear', state_zero: 20, state_one: 5 },
        },
      },
    })
  })

  it('reads exact structural feedback analysis', async () => {
    const analysis = {
      revision: {
        project: 'A', graph_revision: 4, scenario: null,
        dependence_revision: null, formula_revision: 0,
      },
      components: [{
        nodes: ['A', 'B'],
        edges: [{ source: 'A', kind: 'contributes', destination: 'B' }],
        is_feedback: true,
      }],
      cycles: [{
        nodes: ['A', 'B'],
        edges: [
          { source: 'A', kind: 'contributes', destination: 'B' },
          { source: 'B', kind: 'contributes', destination: 'A' },
        ],
      }],
      cycles_truncated: false,
      limits: { maximum_cycle_length: 8, maximum_cycles: 1000 },
    }
    const fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(analysis), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      }),
    )
    vi.stubGlobal('fetch', fetch)
    await expect(api.structuralAnalysis('A')).resolves.toEqual(analysis)
    expect(fetch.mock.calls[0]![0]).toBe('/api/v1/projects/A/analysis/structure')
  })

  it('reads separate topology and evidence impediment orders', async () => {
    const analysis = {
      revision: {
        project: 'A', graph_revision: 4, scenario: null,
        dependence_revision: null, formula_revision: 0,
      },
      topology_candidates: [{
        factor: 'A', controllable: true, reachable_outcomes: ['B'],
        nearest_outcome_distance: 1,
        path_edges: [{ source: 'A', kind: 'contributes', destination: 'B' }],
        direct_evidence: [],
        relationship_evidence: [{
          edge: { source: 'A', kind: 'contributes', destination: 'B' },
          references: ['ADR-1'],
        }],
        unsupported_path_edges: [],
      }],
      evidence_priority: ['A'],
    }
    const fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(analysis), {
        status: 200, headers: { 'Content-Type': 'application/json' },
      }),
    )
    vi.stubGlobal('fetch', fetch)
    await expect(api.impedimentAnalysis('A')).resolves.toEqual(analysis)
    expect(fetch.mock.calls[0]![0]).toBe('/api/v1/projects/A/analysis/impediments')
  })

  it('lists, creates, and analyzes finite-horizon scenarios', async () => {
    const draft = {
      name: 'delivery', title: 'Delivery plan', rationale: '',
      objectives: [{ outcome_id: 'A', direction: 'maximize' as const, importance: 1 }],
      planning_horizon: 12, budgets: [], candidate_interventions: ['B'],
      monte_carlo: {
        seed: 42, minimum_samples: 100, maximum_samples: 1000,
        absolute_tolerance: 0.01, relative_tolerance: 0.01,
      },
    }
    const scenario = { id: 'A', revision: 0, ...draft }
    const analysis = {
      revision: {
        project: 'A', graph_revision: 5, scenario: ['A', 0],
        dependence_revision: null, formula_revision: 0,
      },
      planning_horizon: 12,
      candidates: [],
    }
    const updated = { ...scenario, revision: 1, title: 'Updated delivery plan' }
    const fetch = vi.fn()
      .mockResolvedValueOnce(new Response(JSON.stringify([scenario]), {
        status: 200, headers: { 'Content-Type': 'application/json' },
      }))
      .mockResolvedValueOnce(new Response(JSON.stringify({
        request_id: '00000000-0000-4000-8000-000000000000', project_revision: 8,
        outcome: { type: 'scenario_created', value: scenario },
      }), { status: 201, headers: { 'Content-Type': 'application/json' } }))
      .mockResolvedValueOnce(new Response(JSON.stringify(analysis), {
        status: 200, headers: { 'Content-Type': 'application/json' },
      }))
      .mockResolvedValueOnce(new Response(JSON.stringify({
        request_id: '00000000-0000-4000-8000-000000000000', project_revision: 9,
        outcome: { type: 'scenario_updated', value: updated },
      }), { status: 201, headers: { 'Content-Type': 'application/json' } }))
    vi.stubGlobal('fetch', fetch)
    vi.stubGlobal('crypto', { randomUUID: () => '00000000-0000-4000-8000-000000000000' })
    await expect(api.scenarios('A')).resolves.toEqual([scenario])
    await expect(api.createScenario(project, draft)).resolves.toEqual(scenario)
    await expect(api.scenarioAnalysis('A', 'A')).resolves.toEqual(analysis)
    await expect(api.updateScenario(project, scenario, {
      ...draft,
      title: 'Updated delivery plan',
    })).resolves.toEqual(updated)
    expect(fetch.mock.calls[0]![0]).toBe('/api/v1/projects/A/scenarios')
    expect(JSON.parse((fetch.mock.calls[1]![1] as RequestInit).body as string)).toMatchObject({
      expected_revision: 7,
      command: { type: 'create_scenario', payload: { scenario: draft } },
    })
    expect(fetch.mock.calls[2]![0]).toBe('/api/v1/projects/A/scenarios/A/analysis')
    expect(JSON.parse((fetch.mock.calls[3]![1] as RequestInit).body as string)).toMatchObject({
      expected_revision: 7,
      command: {
        type: 'update_scenario',
        payload: {
          id: 'A',
          expected_revision: 0,
          scenario: { title: 'Updated delivery plan' },
        },
      },
    })
  })

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

  it('sends revision-checked native state configuration', async () => {
    const node = {
      id: 'A', revision: 3, name: 'feedback', normalized_name: 'feedback', title: 'Feedback',
      description: '', aliases: [], metadata: {},
      payload: {
        kind: 'factor' as const,
        properties: { current: null, desired: null, controllable: true, evidence: [] },
      },
    }
    const quantity = {
      unit: 'day', dimension: { day: 1 }, aggregation: null,
      support: { type: 'non_negative' as const }, operational_definition: 'Elapsed time',
    }
    const fetch = vi.fn().mockResolvedValue(new Response(JSON.stringify({
      request_id: '00000000-0000-4000-8000-000000000000',
      project_revision: 8,
      outcome: { type: 'node_quantity_state_set', value: { ...node, native_state: { quantity } } },
    }), { status: 201, headers: { 'Content-Type': 'application/json' } }))
    vi.stubGlobal('fetch', fetch)
    vi.stubGlobal('crypto', { randomUUID: () => '00000000-0000-4000-8000-000000000000' })

    await api.setNodeQuantityState(project, node, { quantity })

    expect(JSON.parse((fetch.mock.calls[0]![1] as RequestInit).body as string)).toMatchObject({
      expected_revision: 7,
      command: {
        type: 'set_node_quantity_state',
        payload: { node: 'A', expected_revision: 3, quantity },
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
      source: { type: 'distribution', distribution: { type: 'beta', alpha: 8, beta: 2 } },
      provenance: ['planning'],
      uncertainty: {
        epistemic: 'Limited evidence',
        process: 'Weekly variation',
        measurement: 'Sampling error',
      },
    })
    expect(JSON.parse((fetch.mock.calls[0]![1] as RequestInit).body as string)).toMatchObject({
      command: {
        type: 'set_estimate',
        payload: {
          address: { project: 'A', owner: { kind: 'node', id: 'A' }, estimate: 'B' },
          slot: { kind: 'desired' },
          distribution: { type: 'beta', alpha: 8, beta: 2 },
          provenance: ['planning'],
          uncertainty: {
            epistemic: 'Limited evidence',
            process: 'Weekly variation',
            measurement: 'Sampling error',
          },
        },
      },
    })
  })

  it('persists Fermi state sources without accepting a client result distribution', async () => {
    const node = {
      id: 'A', revision: 0, name: 'flow', normalized_name: 'flow', title: 'Flow',
      description: '', aliases: [], metadata: {},
      payload: { kind: 'factor' as const, properties: { current: null, desired: null, controllable: false, evidence: [] } },
    }
    const definition = {
      language: 'optimist_squiggle_v1' as const,
      equation: 'confidence',
      variables: [{ name: 'confidence', estimate: 0.5, unit: '', uncertainty: { type: 'three_point' as const, low: 0.4, high: 0.6 } }],
      formula: { type: 'literal' as const, distribution: { type: 'point' as const, value: 0.5 }, unit: {} },
      monte_carlo: { seed: 42, minimum_samples: 100, maximum_samples: 1000, absolute_tolerance: 0.01, relative_tolerance: 0.01 },
    }
    const fetch = vi.fn().mockResolvedValue(new Response(JSON.stringify({
      request_id: 'request', project_revision: 8,
      outcome: {
        type: 'fermi_estimate_set',
        value: {
          address: { project: 'A', owner: { kind: 'node', id: 'A' }, estimate: 'A' },
          slot: { kind: 'current' }, revision: 0,
          distribution: { type: 'point', value: 0.5 },
          source: { type: 'fermi', definition, assessment: {} },
          provenance: [],
        },
      },
    }), { status: 201, headers: { 'Content-Type': 'application/json' } }))
    vi.stubGlobal('fetch', fetch)
    vi.stubGlobal('crypto', { randomUUID: () => 'request' })

    await api.setStateEstimate(project, node, {
      slot: 'current', source: { type: 'fermi', definition }, provenance: [],
    })
    const body = JSON.parse((fetch.mock.calls[0]![1] as RequestInit).body as string)
    expect(body.command).toMatchObject({
      type: 'set_fermi_estimate',
      payload: { slot: { kind: 'current' }, definition },
    })
    expect(body.command.payload).not.toHaveProperty('distribution')
  })

  it('previews and persists Squiggle state sources through backend evaluation', async () => {
    const node = {
      id: 'A', revision: 0, name: 'flow', normalized_name: 'flow', title: 'Flow',
      description: '', aliases: [], metadata: {},
      payload: { kind: 'factor' as const, properties: { current: null, desired: null, controllable: false, evidence: [] } },
    }
    const definition = {
      source: 'beta(8, 2)', seed: 42, sample_count: 512, target_unit: {},
    }
    const fetch = vi.fn()
      .mockResolvedValueOnce(new Response(JSON.stringify({
        assessment: { family: 'Beta', mean: 0.8, variance: 0.01, p05: 0.5, p50: 0.8, p95: 0.98, seed: 42, sample_count: 512 },
        effective_distribution: { type: 'empirical', samples: [0.5, 0.8, 0.98] },
        predictive_checks: { attempted_draws: 512, valid_draws: 512, invalid_draws: 0, support_violation_draws: 0, support_violation_probability: 0, representative_outcomes: [] },
      }), { status: 200, headers: { 'Content-Type': 'application/json' } }))
      .mockResolvedValueOnce(new Response(JSON.stringify({
        request_id: 'request', project_revision: 8,
        outcome: {
          type: 'squiggle_estimate_set',
          value: {
            address: { project: 'A', owner: { kind: 'node', id: 'A' }, estimate: 'A' },
            slot: { kind: 'current' }, revision: 0,
            distribution: { type: 'empirical', samples: [0.5, 0.8, 0.98] },
            source: { type: 'squiggle', definition, assessment: {} }, provenance: [],
          },
        },
      }), { status: 201, headers: { 'Content-Type': 'application/json' } }))
    vi.stubGlobal('fetch', fetch)
    vi.stubGlobal('crypto', { randomUUID: () => 'request' })

    await api.assessSquiggle('A', definition, 'probability')
    await api.setStateEstimate(project, node, {
      slot: 'current', source: { type: 'squiggle', definition }, provenance: [],
    })
    expect(fetch.mock.calls[0]![0]).toBe('/api/v1/projects/A/analysis/squiggle-assessment')
    expect(JSON.parse((fetch.mock.calls[0]![1] as RequestInit).body as string).support).toBe('probability')
    const body = JSON.parse((fetch.mock.calls[1]![1] as RequestInit).body as string)
    expect(body.command).toMatchObject({
      type: 'set_squiggle_estimate',
      payload: { slot: { kind: 'current' }, definition },
    })
    expect(body.command.payload).not.toHaveProperty('distribution')
  })

  it('sets a metric estimate in native units through the current slot', async () => {
    const node = {
      id: 'A', revision: 0, name: 'lead_time', normalized_name: 'lead_time', title: 'Lead time',
      description: '', aliases: [], metadata: {},
      payload: {
        kind: 'metric' as const,
        properties: {
          unit: 'days', aggregation: null,
          support: { type: 'non_negative' as const }, current: null,
        },
      },
    }
    const fetch = vi.fn().mockResolvedValue(new Response(JSON.stringify({
      request_id: 'request', project_revision: 2,
      outcome: {
        type: 'estimate_set',
        value: {
          address: { project: 'A', owner: { kind: 'node', id: 'A' }, estimate: 'A' },
          slot: { kind: 'current' }, revision: 0,
          distribution: { type: 'log_normal', location: 2, scale: 0.3 }, provenance: [],
        },
      },
    }), { status: 201, headers: { 'Content-Type': 'application/json' } }))
    vi.stubGlobal('fetch', fetch)
    vi.stubGlobal('crypto', { randomUUID: () => 'request' })

    await api.setStateEstimate(project, node, {
      slot: 'current',
      source: { type: 'distribution', distribution: { type: 'log_normal', location: 2, scale: 0.3 } },
      provenance: [],
    })
    expect(JSON.parse((fetch.mock.calls[0]![1] as RequestInit).body as string)).toMatchObject({
      command: {
        type: 'set_estimate',
        payload: {
          address: { project: 'A', owner: { kind: 'node', id: 'A' }, estimate: 'A' },
          slot: { kind: 'current' },
          distribution: { type: 'log_normal', location: 2, scale: 0.3 },
        },
      },
    })
  })

  it('sends revision-checked relationship edit and delete commands', async () => {
    const edge = {
      source: 'A',
      source_kind: 'factor' as const,
      destination: 'B',
      destination_kind: 'factor' as const,
      revision: 2,
      description: '',
      metadata: {},
      payload: { kind: 'requires' as const, properties: { hard: true } },
    }
    const updated = { ...edge, revision: 3, description: 'Required first.' }
    const fetch = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            request_id: '00000000-0000-4000-8000-000000000000',
            project_revision: 8,
            outcome: { type: 'edge_metadata_updated', value: updated },
          }),
          { status: 201, headers: { 'Content-Type': 'application/json' } },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            request_id: '00000000-0000-4000-8000-000000000000',
            project_revision: 9,
            outcome: { type: 'edge_deleted', value: updated },
          }),
          { status: 201, headers: { 'Content-Type': 'application/json' } },
        ),
      )
    vi.stubGlobal('fetch', fetch)
    vi.stubGlobal('crypto', {
      randomUUID: () => '00000000-0000-4000-8000-000000000000',
    })

    await api.updateEdge(project, edge, {
      description: 'Required first.',
      metadata: { source: 'ADR-1' },
    })
    await api.deleteEdge(project, updated)
    expect(JSON.parse((fetch.mock.calls[0]![1] as RequestInit).body as string)).toMatchObject({
      command: {
        type: 'update_edge_metadata',
        payload: {
          id: { source: 'A', kind: 'requires', destination: 'B' },
          expected_revision: 2,
          description: 'Required first.',
          metadata: { source: 'ADR-1' },
        },
      },
    })
    expect(JSON.parse((fetch.mock.calls[1]![1] as RequestInit).body as string)).toMatchObject({
      command: {
        type: 'delete_edge',
        payload: { id: { source: 'A', kind: 'requires', destination: 'B' } },
      },
    })
  })

  it('sends project-revision checked node deletion commands', async () => {
    const node = {
      id: 'A', revision: 2, name: 'flow', normalized_name: 'flow', title: 'Flow',
      description: '', aliases: [], metadata: {},
      payload: { kind: 'factor' as const, properties: { current: null, desired: null, controllable: false, evidence: [] } },
    }
    const fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({
        request_id: '00000000-0000-4000-8000-000000000000', project_revision: 8,
        outcome: { type: 'node_deleted', value: node },
      }), { status: 201, headers: { 'Content-Type': 'application/json' } }),
    )
    vi.stubGlobal('fetch', fetch)
    vi.stubGlobal('crypto', { randomUUID: () => '00000000-0000-4000-8000-000000000000' })
    await api.deleteNode(project, node)
    expect(JSON.parse((fetch.mock.calls[0]![1] as RequestInit).body as string)).toMatchObject({
      expected_revision: 7,
      command: { type: 'delete_node', payload: { id: 'A' } },
    })
  })

  it('appends typed observations to their measurement edge', async () => {
    const edge = {
      source: 'A', source_kind: 'metric' as const, destination: 'B',
      destination_kind: 'factor' as const, revision: 0, description: '', metadata: {},
      payload: {
        kind: 'measures' as const,
        properties: { polarity: 'lower_is_better' as const, observations: [] },
      },
    }
    const observation = {
      id: 0, revision: 0, value: 4.2, unit: 'days',
      observed_at: '2026-07-16T12:30:00.000Z', source: 'delivery dashboard',
      measurement_standard_deviation: 0.2, supersedes: null,
    }
    const fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({
        request_id: '00000000-0000-4000-8000-000000000000', project_revision: 8,
        outcome: { type: 'observation_appended', value: { edge, observation } },
      }), { status: 201, headers: { 'Content-Type': 'application/json' } }),
    )
    vi.stubGlobal('fetch', fetch)
    vi.stubGlobal('crypto', { randomUUID: () => '00000000-0000-4000-8000-000000000000' })
    await expect(api.appendObservation(project, edge, {
      value: 4.2, unit: 'days', observed_at: '2026-07-16T12:30:00.000Z',
      source: 'delivery dashboard', measurement_standard_deviation: 0.2,
    })).resolves.toEqual(observation)
    expect(JSON.parse((fetch.mock.calls[0]![1] as RequestInit).body as string)).toMatchObject({
      expected_revision: 7,
      command: {
        type: 'append_observation',
        payload: {
          edge: { source: 'A', kind: 'measures', destination: 'B' },
          observation: { value: 4.2, unit: 'days', source: 'delivery dashboard' },
        },
      },
    })
  })

  it('appends immutable corrections to measurement observations', async () => {
    const edge = {
      source: 'A', source_kind: 'metric' as const, destination: 'B',
      destination_kind: 'factor' as const, revision: 1, description: '', metadata: {},
      payload: {
        kind: 'measures' as const,
        properties: { polarity: 'lower_is_better' as const, observations: [] },
      },
    }
    const observation = {
      id: 1, revision: 0, value: 3.9, unit: 'days',
      observed_at: '2026-07-16T12:30:00.000Z', source: 'delivery dashboard',
      measurement_standard_deviation: 0.2, supersedes: 0,
    }
    const fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({
        request_id: '00000000-0000-4000-8000-000000000000', project_revision: 8,
        outcome: { type: 'observation_corrected', value: { edge, observation } },
      }), { status: 201, headers: { 'Content-Type': 'application/json' } }),
    )
    vi.stubGlobal('fetch', fetch)
    vi.stubGlobal('crypto', { randomUUID: () => '00000000-0000-4000-8000-000000000000' })
    await expect(api.correctObservation(project, edge, {
      observation_id: 0,
      value: 3.9,
    })).resolves.toEqual(observation)
    expect(JSON.parse((fetch.mock.calls[0]![1] as RequestInit).body as string)).toMatchObject({
      expected_revision: 7,
      command: {
        type: 'correct_observation',
        payload: {
          edge: { source: 'A', kind: 'measures', destination: 'B' },
          observation_id: 0,
          value: 3.9,
        },
      },
    })
  })

  it('allocates and removes typed intervention estimates', async () => {
    const node = {
      id: 'B', revision: 0, name: 'automate', normalized_name: 'automate', title: 'Automate',
      description: '', aliases: [], metadata: {},
      payload: {
        kind: 'intervention' as const,
        properties: {
          costs: [{ dimension: 'usd', value: { id: 'A', revision: 0, distribution: { type: 'point' as const, value: 100 } } }],
          duration: null, probability_of_success: null, acceptance_criteria: [],
        },
      },
    }
    const primitive = {
      address: { project: 'A', owner: { kind: 'node', id: 'B' }, estimate: 'B' },
      slot: { kind: 'duration' }, revision: 0,
      distribution: { type: 'log_normal' as const, location: 2, scale: 0.3 }, provenance: ['planning'],
    }
    const fetch = vi.fn()
      .mockResolvedValueOnce(new Response(JSON.stringify({
        request_id: '00000000-0000-4000-8000-000000000000', project_revision: 8,
        outcome: { type: 'estimate_set', value: primitive },
      }), { status: 201, headers: { 'Content-Type': 'application/json' } }))
      .mockResolvedValueOnce(new Response(JSON.stringify({
        request_id: '00000000-0000-4000-8000-000000000000', project_revision: 9,
        outcome: { type: 'estimate_removed', value: primitive },
      }), { status: 201, headers: { 'Content-Type': 'application/json' } }))
    vi.stubGlobal('fetch', fetch)
    vi.stubGlobal('crypto', { randomUUID: () => '00000000-0000-4000-8000-000000000000' })
    await api.setInterventionEstimate(project, node, {
      slot: { kind: 'duration' },
      source: { type: 'distribution', distribution: { type: 'log_normal', location: 2, scale: 0.3 } },
      provenance: ['planning'],
    })
    await api.removeInterventionEstimate(project, node, { id: 'B', revision: 0, distribution: primitive.distribution })
    expect(JSON.parse((fetch.mock.calls[0]![1] as RequestInit).body as string)).toMatchObject({
      command: { type: 'set_estimate', payload: {
        address: { project: 'A', owner: { kind: 'node', id: 'B' }, estimate: 'B' },
        slot: { kind: 'duration' }, distribution: { type: 'log_normal', location: 2, scale: 0.3 },
      } },
    })
    expect(JSON.parse((fetch.mock.calls[1]![1] as RequestInit).body as string)).toMatchObject({
      command: { type: 'remove_estimate', payload: {
        address: { project: 'A', owner: { kind: 'node', id: 'B' }, estimate: 'B' },
      } },
    })
  })

  it('creates, revision-updates, and deletes node-owned evidence', async () => {
    const evidence = { id: 1, revision: 0, summary: 'Queueing observed', source: 'dashboard' }
    const updated = { ...evidence, revision: 1, summary: 'Queueing confirmed' }
    const node = {
      id: 'A', revision: 1, name: 'flow', normalized_name: 'flow', title: 'Flow',
      description: '', aliases: [], metadata: {},
      payload: {
        kind: 'factor' as const,
        properties: { current: null, desired: null, controllable: false, evidence: [evidence] },
      },
    }
    const fetch = vi.fn()
      .mockResolvedValueOnce(new Response(JSON.stringify({
        request_id: '00000000-0000-4000-8000-000000000000', project_revision: 8,
        outcome: { type: 'evidence_created', value: { node, evidence } },
      }), { status: 201, headers: { 'Content-Type': 'application/json' } }))
      .mockResolvedValueOnce(new Response(JSON.stringify({
        request_id: '00000000-0000-4000-8000-000000000000', project_revision: 9,
        outcome: { type: 'evidence_updated', value: { node, evidence: updated } },
      }), { status: 201, headers: { 'Content-Type': 'application/json' } }))
      .mockResolvedValueOnce(new Response(JSON.stringify({
        request_id: '00000000-0000-4000-8000-000000000000', project_revision: 10,
        outcome: { type: 'evidence_deleted', value: { node, evidence: updated } },
      }), { status: 201, headers: { 'Content-Type': 'application/json' } }))
    vi.stubGlobal('fetch', fetch)
    vi.stubGlobal('crypto', { randomUUID: () => '00000000-0000-4000-8000-000000000000' })
    await api.createEvidence(project, node, { summary: evidence.summary, source: evidence.source })
    await api.updateEvidence(project, node, evidence, { summary: updated.summary, source: null })
    await api.deleteEvidence(project, node, updated)
    expect(JSON.parse((fetch.mock.calls[0]![1] as RequestInit).body as string)).toMatchObject({
      command: { type: 'create_evidence', payload: {
        node: 'A', summary: 'Queueing observed', source: 'dashboard',
      } },
    })
    expect(JSON.parse((fetch.mock.calls[1]![1] as RequestInit).body as string)).toMatchObject({
      command: { type: 'update_evidence', payload: {
        node: 'A', evidence_id: 1, expected_revision: 0, summary: 'Queueing confirmed', source: null,
      } },
    })
    expect(JSON.parse((fetch.mock.calls[2]![1] as RequestInit).body as string)).toMatchObject({
      command: { type: 'delete_evidence', payload: {
        node: 'A', evidence_id: 1, expected_revision: 1,
      } },
    })
  })

  it('replaces required and removes optional edge estimates', async () => {
    const effect = { id: 'A', revision: 0, distribution: { type: 'point' as const, value: 0.4 } }
    const lag = { id: 'B', revision: 0, distribution: { type: 'point' as const, value: 2 } }
    const edge = {
      source: 'A', source_kind: 'factor' as const, destination: 'B', destination_kind: 'outcome' as const,
      revision: 0, description: '', metadata: {},
      payload: { kind: 'contributes' as const, properties: { effect, lag, mechanism: '', evidence: [] } },
    }
    const primitive = {
      address: { project: 'A', owner: { kind: 'edge', id: { source: 'A', kind: 'contributes', destination: 'B' } }, estimate: 'A' },
      slot: { kind: 'effect' }, revision: 1,
      distribution: { type: 'scaled_beta' as const, alpha: 3, beta: 2, lower: -1, upper: 1 }, provenance: ['analysis'],
    }
    const fetch = vi.fn()
      .mockResolvedValueOnce(new Response(JSON.stringify({
        request_id: '00000000-0000-4000-8000-000000000000', project_revision: 8,
        outcome: { type: 'estimate_set', value: primitive },
      }), { status: 201, headers: { 'Content-Type': 'application/json' } }))
      .mockResolvedValueOnce(new Response(JSON.stringify({
        request_id: '00000000-0000-4000-8000-000000000000', project_revision: 9,
        outcome: { type: 'estimate_removed', value: { ...primitive, address: { ...primitive.address, estimate: 'B' }, slot: { kind: 'lag' } } },
      }), { status: 201, headers: { 'Content-Type': 'application/json' } }))
    vi.stubGlobal('fetch', fetch)
    vi.stubGlobal('crypto', { randomUUID: () => '00000000-0000-4000-8000-000000000000' })
    await api.setEdgeEstimate(project, edge, {
      slot: { kind: 'effect' }, source: { type: 'distribution', distribution: primitive.distribution }, provenance: ['analysis'],
    })
    await api.removeEdgeEstimate(project, edge, lag)
    expect(JSON.parse((fetch.mock.calls[0]![1] as RequestInit).body as string)).toMatchObject({
      command: { type: 'set_estimate', payload: {
        address: { project: 'A', owner: { kind: 'edge', id: { source: 'A', kind: 'contributes', destination: 'B' } }, estimate: 'A' },
        slot: { kind: 'effect' }, distribution: { type: 'scaled_beta', lower: -1, upper: 1 },
      } },
    })
    expect(JSON.parse((fetch.mock.calls[1]![1] as RequestInit).body as string)).toMatchObject({
      command: { type: 'remove_estimate', payload: {
        address: { project: 'A', owner: { kind: 'edge', id: { source: 'A', kind: 'contributes', destination: 'B' } }, estimate: 'B' },
      } },
    })
  })

  it('replaces a native destination response estimate', async () => {
    const destinationChange = { id: 'A', revision: 0, distribution: { type: 'point' as const, value: -2 } }
    const edge = {
      source: 'A', source_kind: 'factor' as const, destination: 'B', destination_kind: 'metric' as const,
      revision: 0, description: '', metadata: {},
      payload: {
        kind: 'contributes' as const,
        properties: {
          response: {
            source_change: 0.1, source_unit: {}, destination_change: destinationChange,
            destination_unit: { day: 1 },
          },
          lag: null, mechanism: '', evidence: [],
        },
      },
    }
    const fetch = vi.fn().mockResolvedValue(new Response(JSON.stringify({
      request_id: 'request', project_revision: 8,
      outcome: {
        type: 'estimate_set',
        value: {
          address: { project: 'A', owner: { kind: 'edge', id: { source: 'A', kind: 'contributes', destination: 'B' } }, estimate: 'A' },
          slot: { kind: 'response' }, revision: 1,
          distribution: { type: 'normal', mean: -2, standard_deviation: 0.5 }, provenance: [],
        },
      },
    }), { status: 201, headers: { 'Content-Type': 'application/json' } }))
    vi.stubGlobal('fetch', fetch)
    vi.stubGlobal('crypto', { randomUUID: () => 'request' })

    await api.setEdgeEstimate(project, edge, {
      slot: { kind: 'response' },
      source: { type: 'distribution', distribution: { type: 'normal', mean: -2, standard_deviation: 0.5 } },
      provenance: [],
    })
    expect(JSON.parse((fetch.mock.calls[0]![1] as RequestInit).body as string)).toMatchObject({
      command: {
        type: 'set_estimate',
        payload: {
          address: { estimate: 'A' },
          slot: { kind: 'response' },
          distribution: { type: 'normal', mean: -2, standard_deviation: 0.5 },
        },
      },
    })
  })
})
