import { expect, test as base, type Page } from '@playwright/test'

export interface FixtureState {
  revision: number
  project: { id: string; name: string; revision: number } | null
  projects?: Array<{ id: string; name: string; revision: number }>
  nodes: Array<Record<string, unknown>>
  edges: Array<Record<string, unknown>>
  scenarios?: Array<Record<string, unknown>>
  dependence?: Record<string, unknown> | null
}

/**
 * Wraps an authored Squiggle definition the way the server returns it.
 *
 * Commands accept bare definitions but responses carry full estimates, so the
 * mock must convert; otherwise the workbench never exercises the round trip it
 * performs against a real server.
 */
function estimateOf(definition: unknown) {
  return definition === null || definition === undefined
    ? null
    : { id: 'A', revision: 0, source: { type: 'squiggle', definition }, provenance: [] }
}

function releaseOf(release: { type: string; over?: unknown; half_life?: unknown }) {
  if (release.type === 'linear') return { type: 'linear', over: estimateOf(release.over) }
  if (release.type === 'exponential') {
    return { type: 'exponential', half_life: estimateOf(release.half_life) }
  }
  return { type: 'immediate' }
}

function transience(profile: Record<string, any> | null) {
  if (!profile) return null
  const aftereffect = profile.aftereffect
  const release = releaseOf(profile.release)
  return {
    profile: {
      ramp: estimateOf(profile.ramp),
      hold: estimateOf(profile.hold),
      // A profile's immediate release is its default, which the server omits.
      // An aftereffect's release is always written, so it stays explicit here.
      ...(release.type === 'immediate' ? {} : { release }),
      aftereffect: aftereffect
        ? { hold: estimateOf(aftereffect.hold), release: releaseOf(aftereffect.release) }
        : null,
    },
    rebound: estimateOf(aftereffect?.magnitude ?? null),
  }
}

export async function mockApi(page: Page, state: FixtureState) {
  await page.route('**/api/v1/**', async (route) => {
    const request = route.request()
    const url = new URL(request.url())
    const json = (value: unknown, status = 200) =>
      route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(value) })

      if (url.pathname === '/api/v1/health') {
        return json({ status: 'ok', version: '0.1.0', persistence: { state: 'idle' } })
      }
    if (url.pathname === '/api/v1/projects' && request.method() === 'GET') {
      return json(state.projects ?? (state.project ? [state.project] : []))
    }
    if (url.pathname === '/api/v1/projects' && request.method() === 'POST') {
      const projects = state.projects ?? (state.project ? [state.project] : [])
      state.project = {
        id: String.fromCharCode(65 + projects.length),
        name: JSON.parse(request.postData()!).name,
        revision: 0,
      }
      state.projects = [...projects, state.project]
      return json(state.project, 201)
    }
    if (url.pathname === '/api/v1/projects/A' && request.method() === 'GET') {
      if (!state.project) {
        return json({ error: { code: 'project_not_found', message: 'missing', advice: [] } }, 404)
      }
      return json({ ...state.project, revision: state.revision })
    }
    if (url.pathname === '/api/v1/projects/A/nodes') return json(state.nodes)
    if (url.pathname === '/api/v1/projects/A/edges') return json(state.edges)
    if (url.pathname === '/api/v1/projects/A/dependence') {
      return state.dependence
        ? json(state.dependence)
        : json(
            {
              error: {
                code: 'dependence_not_found',
                message: 'no dependence document',
                advice: [],
              },
            },
            404,
          )
    }
    if (url.pathname === '/api/v1/projects/A/scenarios') return json(state.scenarios ?? [])
    if (url.pathname === '/api/v1/projects/A/scenarios/A/analysis') {
      const scenario = state.scenarios?.[0] as {
        id: string
        revision: number
        planning_horizon: number
        objectives: Array<{ outcome_id: string; direction: string; importance: number }>
        candidate_interventions: string[]
        monte_carlo: { seed: number; minimum_samples: number; maximum_samples: number; absolute_tolerance: number; relative_tolerance: number }
      }
      const estimate = { mean: 0.12, variance: 0.02, mean_standard_error: 0.004, variance_standard_error: 0.003 }
      return json({
        revision: { project: 'A', graph_revision: state.revision, scenario: [scenario.id, scenario.revision], dependence_revision: null },
        planning_horizon: scenario.planning_horizon,
        candidates: scenario.candidate_interventions.map((intervention) => ({
          intervention,
          prerequisites: [],
          blocking_requirements: [],
          synergies: [],
          conflicts: [],
          execution_duration: estimate,
          execution_success: { ...estimate, mean: 1, variance: 0 },
          objectives: scenario.objectives.map((objective) => ({
            outcome: objective.outcome_id,
            direction: objective.direction,
            importance: objective.importance,
            reachable: true,
            periods_to_effect: 2,
            baseline: { ...estimate, mean: 0.5 },
            final_state: { ...estimate, mean: 0.62 },
            improvement: estimate,
            trajectory: Array.from({ length: scenario.planning_horizon + 1 }, (_, period) => ({
              period,
              state: { ...estimate, mean: 0.5 + 0.12 * period / scenario.planning_horizon },
              improvement: { ...estimate, mean: 0.12 * period / scenario.planning_horizon },
            })),
          })),
          improvement_covariance: [[0.02]],
          clamped_state_updates: 0,
          diagnostics: {
            seed: scenario.monte_carlo.seed,
            attempted_samples: 120,
            valid_samples: 120,
            invalid_samples: { non_finite_primitive: 0, non_finite_result: 0 },
            criterion: scenario.monte_carlo,
            status: 'converged',
          },
        })),
        feedback_loops: [],
      })
    }
    if (url.pathname === '/api/v1/projects/A/analysis/structure') {
      const causal = state.edges.filter((edge) =>
        ['contributes', 'changes', 'blocks'].includes((edge.payload as { kind: string }).kind),
      )
      const forward = causal.find((edge) => edge.source === 'A' && edge.destination === 'B')
      const backward = causal.find((edge) => edge.source === 'B' && edge.destination === 'A')
      const cycleEdges = forward && backward
        ? [forward, backward].map((edge) => ({
            source: edge.source,
            kind: (edge.payload as { kind: string }).kind,
            destination: edge.destination,
          }))
        : []
      return json({
        revision: { project: 'A', graph_revision: state.revision, scenario: null, dependence_revision: null },
        components: cycleEdges.length
          ? [{ nodes: ['A', 'B'], edges: cycleEdges, is_feedback: true }]
          : state.nodes.map((node) => ({ nodes: [node.id], edges: [], is_feedback: false })),
        cycles: cycleEdges.length ? [{ nodes: ['A', 'B'], edges: cycleEdges }] : [],
        cycles_truncated: false,
        limits: { maximum_cycle_length: 8, maximum_cycles: 1000 },
      })
    }
    if (url.pathname === '/api/v1/projects/A/analysis/impediments') {
      const candidates = state.nodes
        .filter((node) => (node.payload as { kind: string }).kind === 'intervention')
        .map((node) => ({
          intervention: node.id,
          execution_steps: [{ intervention: node.id, duration: null, probability_of_success: null }],
          blocking_requirements: [], synergies: [], conflicts: [],
          expected_duration: 0, expected_success_probability: 1,
        }))
      return json({
        revision: { project: 'A', graph_revision: state.revision, scenario: null, dependence_revision: null },
        candidates,
      })
    }
      if (url.pathname === '/api/v1/projects/A/analysis/squiggle-assessment' && request.method() === 'POST') {
        const { definition } = JSON.parse(request.postData()!)
        const point = definition.source.match(/pointMass\((-?[\d.]+)\)/)?.[1]
        const family = point ? 'PointMass' : definition.source.includes('beta(') ? 'Beta' : definition.source.includes('lognormal(') ? 'Lognormal' : 'SampleSet'
        const samples = point
          ? [Number(point), Number(point), Number(point)]
          : family === 'Beta'
            ? [0.2, 0.5, 0.8]
            : family === 'Lognormal'
              ? [0.5, 1, 2]
              : [-1, 0, 1]
        const sorted = [...samples].sort((left, right) => left - right)
        const mean = samples.reduce((total, value) => total + value, 0) / samples.length
        const variance = samples.reduce((total, value) => total + (value - mean) ** 2, 0) / samples.length
        return json({
          assessment: {
            family, mean, variance,
            p05: sorted[0], p50: sorted[Math.floor(sorted.length / 2)], p95: sorted.at(-1),
            seed: definition.seed, sample_count: definition.sample_count,
          },
          effective_distribution: { type: 'empirical', samples },
          predictive_checks: {
            attempted_draws: samples.length,
            valid_draws: samples.length,
            invalid_draws: 0,
            support_violation_draws: 0,
            support_violation_probability: 0,
            support_compatible: true,
            support_requirement: 'any finite real value',
            representative_outcomes: [
              { percentile: 0.1, value: sorted[0] },
              { percentile: 0.5, value: sorted[Math.floor(sorted.length / 2)] },
              { percentile: 0.9, value: sorted.at(-1) },
            ],
          },
        })
      }
    if (url.pathname === '/api/v1/projects/A/commands' && request.method() === 'POST') {
      const command = JSON.parse(request.postData()!)
      const input = command.command.payload
      if (command.command.type === 'set_project_dependence') {
        const revision = state.dependence ? (state.dependence.revision as number) + 1 : 0
        state.dependence = { ...input.model, revision }
        state.revision += 1
        return json({ request_id: command.request_id, project_revision: state.revision, outcome: { type: 'project_dependence_set', value: state.dependence } }, 201)
      }
      if (command.command.type === 'set_state_relation') {
        const node = state.nodes.find((node) => node.id === input.node)!
        if ((node.payload as { kind: string }).kind === 'metric') {
          (node.payload as { properties: Record<string, unknown> }).properties.relation =
            input.relation
        } else {
          (node.native_state as Record<string, unknown>).relation = input.relation
        }
        node.revision = (node.revision as number) + 1
        state.revision += 1
        return json({ request_id: command.request_id, project_revision: state.revision, outcome: { type: 'state_relation_set', value: node } }, 201)
      }
      if (command.command.type === 'create_scenario') {
        const scenario = { id: 'A', revision: 0, ...input.scenario }
        state.scenarios = [...(state.scenarios ?? []), scenario]
        state.revision += 1
        return json({ request_id: command.request_id, project_revision: state.revision, outcome: { type: 'scenario_created', value: scenario } }, 201)
      }
      if (command.command.type === 'update_scenario') {
        const current = state.scenarios?.find((scenario) => scenario.id === input.id) as { id: string; revision: number }
        const scenario = { id: current.id, revision: current.revision + 1, ...input.scenario }
        state.scenarios = state.scenarios?.map((value) => value.id === scenario.id ? scenario : value)
        state.revision += 1
        return json({ request_id: command.request_id, project_revision: state.revision, outcome: { type: 'scenario_updated', value: scenario } }, 201)
      }
      if (command.command.type === 'create_edge') {
        const source = state.nodes.find((node) => node.id === input.source)!
        const destination = state.nodes.find((node) => node.id === input.destination)!
        const edge = {
          source: input.source,
          source_kind: (source.payload as { kind: string }).kind,
          destination: input.destination,
          destination_kind: (destination.payload as { kind: string }).kind,
          revision: 0,
          description: '',
          metadata: {},
          payload: input.payload,
        }
        state.edges.push(edge)
        state.revision += 1
        return json({ request_id: command.request_id, project_revision: state.revision, outcome: { type: 'edge_created', value: edge } }, 201)
      }
      if (command.command.type === 'delete_edge') {
        const index = state.edges.findIndex((edge) =>
          edge.source === input.id.source &&
          edge.destination === input.id.destination &&
          (edge.payload as { kind: string }).kind === input.id.kind,
        )
        const [edge] = state.edges.splice(index, 1)
        state.revision += 1
        return json({ request_id: command.request_id, project_revision: state.revision, outcome: { type: 'edge_deleted', value: edge } }, 201)
      }
      if (command.command.type === 'set_effect_profile' || command.command.type === 'update_causal_effect') {
        const edge = state.edges.find((edge) =>
          edge.source === input.edge.source &&
          edge.destination === input.edge.destination &&
          (edge.payload as { kind: string }).kind === input.edge.kind,
        )!
        const properties = (edge.payload as { properties: Record<string, unknown> }).properties
        if (command.command.type === 'set_effect_profile') {
          properties.transience = transience(input.profile)
        } else {
          properties.mechanism = input.mechanism
          properties.evidence = input.evidence
        }
        edge.revision += 1
        state.revision += 1
        const type = command.command.type === 'set_effect_profile' ? 'effect_profile_set' : 'causal_effect_updated'
        return json({ request_id: command.request_id, project_revision: state.revision, outcome: { type, value: edge } }, 201)
      }
      if (command.command.type === 'set_node_quantity_state') {
        const node = state.nodes.find((node) => node.id === input.node)!
        const existing = node.native_state as { current?: unknown; forecast?: unknown } | undefined
        node.native_state = {
          quantity: input.quantity,
          current: existing?.current ?? null,
          forecast: existing?.forecast ?? null,
        }
        node.revision = (node.revision as number) + 1
        state.revision += 1
        return json({ request_id: command.request_id, project_revision: state.revision, outcome: { type: 'node_quantity_state_set', value: node } }, 201)
      }
      if (command.command.type === 'delete_node') {
        const index = state.nodes.findIndex((node) => node.id === input.id)
        const [node] = state.nodes.splice(index, 1)
        state.revision += 1
        return json({ request_id: command.request_id, project_revision: state.revision, outcome: { type: 'node_deleted', value: node } }, 201)
      }
      const node = {
        id: String.fromCharCode(65 + state.nodes.length),
        revision: 0,
        name: input.name,
        normalized_name: input.name,
        title: input.title,
        description: '',
        aliases: [],
        metadata: {},
        payload: input.payload,
      }
      state.nodes.push(node)
      state.revision += 1
      return json({ request_id: command.request_id, project_revision: state.revision, outcome: { type: 'node_created', value: node } }, 201)
    }
    return json({ error: { code: 'not_found', message: url.pathname, advice: [] } }, 404)
  })
}

export async function expectCanvasPainted(page: Page) {
  await expect
    .poll(async () =>
      page.locator('.graph-canvas canvas[data-id="layer2-node"]').evaluate((canvas) => {
        const context = (canvas as HTMLCanvasElement).getContext('2d')
        if (!context) return 0
        const { width, height } = canvas as HTMLCanvasElement
        const data = context.getImageData(0, 0, width, height).data
        let nonTransparent = 0
        for (let index = 3; index < data.length; index += 4) {
          if (data[index] > 0) nonTransparent += 1
        }
        return nonTransparent
      }),
    )
    .toBeGreaterThan(100)
}

export const test = base.extend<{ apiState: FixtureState }>({
  apiState: [
    async ({ page }, use) => {
      const state: FixtureState = { project: null, revision: 0, nodes: [], edges: [] }
      await mockApi(page, state)
      await use(state)
    },
    { auto: true },
  ],
})

export { expect }
