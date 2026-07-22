import { expect, test as base, type Page } from '@playwright/test'

export interface FixtureState {
  revision: number
  project: { id: string; name: string; revision: number } | null
  projects?: Array<{ id: string; name: string; revision: number }>
  nodes: Array<Record<string, unknown>>
  edges: Array<Record<string, unknown>>
  scenarios?: Array<Record<string, unknown>>
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
        revision: { project: 'A', graph_revision: state.revision, scenario: [scenario.id, scenario.revision], dependence_revision: null, formula_revision: 0 },
        planning_horizon: scenario.planning_horizon,
        candidates: scenario.candidate_interventions.map((intervention) => ({
          intervention,
          objectives: scenario.objectives.map((objective) => ({
            outcome: objective.outcome_id,
            direction: objective.direction,
            importance: objective.importance,
            reachable: true,
            baseline: estimate,
            final_state: estimate,
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
            invalid_samples: { zero_denominator: 0, non_finite_primitive: 0, non_finite_result: 0 },
            criterion: scenario.monte_carlo,
            status: 'converged',
          },
        })),
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
        revision: { project: 'A', graph_revision: state.revision, scenario: null, dependence_revision: null, formula_revision: 0 },
        components: cycleEdges.length
          ? [{ nodes: ['A', 'B'], edges: cycleEdges, is_feedback: true }]
          : state.nodes.map((node) => ({ nodes: [node.id], edges: [], is_feedback: false })),
        cycles: cycleEdges.length ? [{ nodes: ['A', 'B'], edges: cycleEdges }] : [],
        cycles_truncated: false,
        limits: { maximum_cycle_length: 8, maximum_cycles: 1000 },
      })
    }
    if (url.pathname === '/api/v1/projects/A/analysis/impediments') {
      const factors = state.nodes.filter((node) => (node.payload as { kind: string }).kind === 'factor')
      const outcomes = new Set(
        state.nodes
          .filter((node) => (node.payload as { kind: string }).kind === 'outcome')
          .map((node) => node.id),
      )
      const candidates = factors.flatMap((node) => {
        const pathEdges = state.edges.filter((edge) =>
          edge.source === node.id && outcomes.has(edge.destination) &&
          ['contributes', 'blocks'].includes((edge.payload as { kind: string }).kind),
        )
        if (!pathEdges.length) return []
        const evidence = (node.payload as { properties: { evidence?: unknown[] } }).properties.evidence ?? []
        const relationshipEvidence = pathEdges.flatMap((edge) => {
          const references = (edge.payload as { properties?: { evidence?: string[] } }).properties?.evidence ?? []
          return references.length
            ? [{ edge: { source: edge.source, kind: (edge.payload as { kind: string }).kind, destination: edge.destination }, references }]
            : []
        })
        const evidenced = new Set(
          relationshipEvidence.map((value) => `${value.edge.source}:${value.edge.kind}:${value.edge.destination}`),
        )
        const typedEdges = pathEdges.map((edge) => ({
          source: edge.source,
          kind: (edge.payload as { kind: string }).kind,
          destination: edge.destination,
        }))
        return [{
          factor: node.id,
          controllable: Boolean((node.payload as { properties: { controllable?: boolean } }).properties.controllable),
          reachable_outcomes: pathEdges.map((edge) => edge.destination).sort(),
          nearest_outcome_distance: 1,
          path_edges: typedEdges,
          direct_evidence: evidence,
          relationship_evidence: relationshipEvidence,
          unsupported_path_edges: typedEdges.filter((edge) =>
            !evidenced.has(`${edge.source}:${edge.kind}:${edge.destination}`),
          ),
        }]
      }).sort((left, right) =>
        right.reachable_outcomes.length - left.reachable_outcomes.length || left.factor.localeCompare(right.factor),
      )
      const evidencePriority = [...candidates]
        .sort((left, right) =>
          right.direct_evidence.length - left.direct_evidence.length ||
          right.relationship_evidence.length - left.relationship_evidence.length ||
          candidates.indexOf(left) - candidates.indexOf(right),
        )
        .map((candidate) => candidate.factor)
      return json({
        revision: { project: 'A', graph_revision: state.revision, scenario: null, dependence_revision: null, formula_revision: 0 },
        topology_candidates: candidates,
        evidence_priority: evidencePriority,
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
        })
      }
    if (url.pathname === '/api/v1/projects/A/commands' && request.method() === 'POST') {
      const command = JSON.parse(request.postData()!)
      const input = command.command.payload
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
