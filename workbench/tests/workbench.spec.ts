import { expect, test, type Page } from '@playwright/test'

interface FixtureState {
  revision: number
  project: { id: string; name: string; revision: number } | null
  projects?: Array<{ id: string; name: string; revision: number }>
  nodes: Array<Record<string, unknown>>
  edges: Array<Record<string, unknown>>
  scenarios?: Array<Record<string, unknown>>
}

async function mockApi(page: Page, state: FixtureState) {
  await page.route('**/api/v1/**', async (route) => {
    const request = route.request()
    const url = new URL(request.url())
    const json = (value: unknown, status = 200) =>
      route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(value) })

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
      if (!state.project) return json({ error: { code: 'project_not_found', message: 'missing', advice: [] } }, 404)
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
            outcome: objective.outcome_id, direction: objective.direction,
            importance: objective.importance, reachable: true,
            baseline: estimate, final_state: estimate, improvement: estimate,
          })),
          improvement_covariance: [[0.02]], clamped_state_updates: 0,
          diagnostics: {
            seed: scenario.monte_carlo.seed, attempted_samples: 120, valid_samples: 120,
            invalid_samples: { zero_denominator: 0, non_finite_primitive: 0, non_finite_result: 0 },
            criterion: scenario.monte_carlo, status: 'converged',
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
      const outcomes = new Set(state.nodes.filter((node) => (node.payload as { kind: string }).kind === 'outcome').map((node) => node.id))
      const candidates = factors.flatMap((node) => {
        const pathEdges = state.edges.filter((edge) =>
          edge.source === node.id && outcomes.has(edge.destination) &&
          ['contributes', 'blocks'].includes((edge.payload as { kind: string }).kind),
        )
        if (!pathEdges.length) return []
        const evidence = ((node.payload as { properties: { evidence?: unknown[] } }).properties.evidence ?? [])
        const relationshipEvidence = pathEdges.flatMap((edge) => {
          const references = (edge.payload as { properties?: { evidence?: string[] } }).properties?.evidence ?? []
          return references.length ? [{ edge: { source: edge.source, kind: (edge.payload as { kind: string }).kind, destination: edge.destination }, references }] : []
        })
        const evidenced = new Set(relationshipEvidence.map((value) => `${value.edge.source}:${value.edge.kind}:${value.edge.destination}`))
        return [{
          factor: node.id,
          controllable: Boolean((node.payload as { properties: { controllable?: boolean } }).properties.controllable),
          reachable_outcomes: pathEdges.map((edge) => edge.destination).sort(),
          nearest_outcome_distance: 1,
          path_edges: pathEdges.map((edge) => ({ source: edge.source, kind: (edge.payload as { kind: string }).kind, destination: edge.destination })),
          direct_evidence: evidence,
          relationship_evidence: relationshipEvidence,
          unsupported_path_edges: pathEdges
            .map((edge) => ({ source: edge.source, kind: (edge.payload as { kind: string }).kind, destination: edge.destination }))
            .filter((edge) => !evidenced.has(`${edge.source}:${edge.kind}:${edge.destination}`)),
        }]
      }).sort((left, right) => right.reachable_outcomes.length - left.reachable_outcomes.length || left.factor.localeCompare(right.factor))
      const evidencePriority = [...candidates].sort((left, right) =>
        right.direct_evidence.length - left.direct_evidence.length ||
        right.relationship_evidence.length - left.relationship_evidence.length ||
        candidates.indexOf(left) - candidates.indexOf(right),
      ).map((candidate) => candidate.factor)
      return json({
        revision: { project: 'A', graph_revision: state.revision, scenario: null, dependence_revision: null, formula_revision: 0 },
        topology_candidates: candidates,
        evidence_priority: evidencePriority,
      })
    }
    if (url.pathname === '/api/v1/projects/A/commands' && request.method() === 'POST') {
      const command = JSON.parse(request.postData()!)
      const input = command.command.payload
      if (command.command.type === 'create_scenario') {
        const scenario = { id: 'A', revision: 0, ...input.scenario }
        state.scenarios = [...(state.scenarios ?? []), scenario]
        state.revision += 1
        return json({
          request_id: command.request_id,
          project_revision: state.revision,
          outcome: { type: 'scenario_created', value: scenario },
        }, 201)
      }
      if (command.command.type === 'update_scenario') {
        const current = state.scenarios?.find((scenario) => scenario.id === input.id) as { id: string; revision: number }
        const scenario = { id: current.id, revision: current.revision + 1, ...input.scenario }
        state.scenarios = state.scenarios?.map((value) => value.id === scenario.id ? scenario : value)
        state.revision += 1
        return json({
          request_id: command.request_id,
          project_revision: state.revision,
          outcome: { type: 'scenario_updated', value: scenario },
        }, 201)
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
        return json(
          {
            request_id: command.request_id,
            project_revision: state.revision,
            outcome: { type: 'edge_created', value: edge },
          },
          201,
        )
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
      return json(
        {
          request_id: command.request_id,
          project_revision: state.revision,
          outcome: { type: 'node_created', value: node },
        },
        201,
      )
    }
    return json({ error: { code: 'not_found', message: url.pathname, advice: [] } }, 404)
  })
}

async function expectCanvasPainted(page: Page) {
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

test.beforeEach(async ({ page }) => {
  await mockApi(page, { project: null, revision: 0, nodes: [], edges: [] })
})

test('creates nodes and a relationship, then filters and inspects the model', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop workflow assertion')
  await page.goto('/')
  await expect(page.getByRole('heading', { name: 'Create your first project' })).toBeVisible()
  await page.getByRole('button', { name: 'Create project' }).click()
  await page.getByLabel('Project name').fill('Delivery reliability')
  await page.getByRole('button', { name: 'Create project' }).last().click()

  await expect(page.getByRole('heading', { name: 'Start with a system element' })).toBeVisible()
  await page.getByRole('button', { name: 'Add node' }).last().click()
  await page.getByLabel('Title').fill('Fast feedback')
  await page.getByLabel('Directly controllable').check()
  await page.getByRole('button', { name: 'Add node' }).last().click()

  await page.getByRole('button', { name: 'Add node' }).first().click()
  await page.getByLabel('Title').fill('Learning rate')
  await page.getByRole('button', { name: 'Add node' }).last().click()

  await page.getByRole('button', { name: 'Relationship', exact: true }).click()
  await page.getByRole('form', { name: 'Add relationship' }).getByRole('combobox').first().selectOption('part_of')
  await page.getByLabel('Source').selectOption('A')
  await page.getByLabel('Destination').selectOption('B')
  await page.getByRole('button', { name: 'Add relationship' }).click()

  await expect(page.getByTestId('graph-surface')).toBeVisible()
  await expect(page.getByRole('button', { name: /Fast feedback/ })).toBeVisible()
  await page.getByRole('button', { name: /Fast feedback/ }).click()
  await expect(page.getByRole('heading', { name: 'Fast feedback' })).toBeVisible()
  await expect(page.getByText('Controllable')).toBeVisible()
  await expect(page.getByText('1 relationships')).toBeVisible()
  const selectedOutlineNode = page.getByLabel('Node outline').getByRole('button', { name: /Fast feedback/ })
  await selectedOutlineNode.focus()
  await selectedOutlineNode.press('ArrowDown')
  await expect(page.getByRole('heading', { name: 'Learning rate' })).toBeVisible()
  await page.getByRole('button', { name: 'Table view' }).click()
  const nodeTable = page.getByRole('table', { name: 'Visible graph nodes' })
  await expect(nodeTable.getByRole('button', { name: 'Learning rate' })).toHaveAttribute('aria-current', 'true')
  await expect(nodeTable.getByRole('row', { name: /B Learning rate factor/ })).toHaveClass(/selected/)
  await expectCanvasPainted(page)
  await page.screenshot({ path: 'artifacts/workbench-desktop.png', fullPage: true })

  await page.getByLabel('Search graph').fill('missing')
  await expect(page.getByText('0 nodes')).toBeVisible()
  await expect(page.getByText('0 relationships')).toBeVisible()
  await page.getByLabel('Search graph').fill('feedback')
  await expect(page.getByText('1 nodes')).toBeVisible()
})

test('creates another project from the project dropdown', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop workflow assertion')
  await page.goto('/')
  await page.getByRole('button', { name: 'Create project' }).click()
  await page.getByLabel('Project name').fill('Existing model')
  await page.getByRole('button', { name: 'Create project' }).last().click()
  const projectSelect = page.getByLabel('Project', { exact: true })
  await expect(projectSelect).toHaveValue('A')

  await projectSelect.selectOption({ label: 'New project...' })
  await expect(page.getByRole('heading', { name: 'Create project' })).toBeVisible()
  await expect(projectSelect).toHaveValue('A')
  await page.getByLabel('Project name').fill('Second model')
  await page.getByRole('button', { name: 'Create project' }).last().click()

  await expect(projectSelect).toHaveValue('B')
  await expect(projectSelect.locator('option')).toHaveText([
    'Existing model',
    'Second model',
    'New project...',
  ])
})

test('analyzes and highlights causal feedback loops', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop workflow assertion')
  const estimate = { id: 'A', revision: 0, distribution: { type: 'point', value: 0.5 }, provenance: [] }
  const nodes = ['A', 'B'].map((id) => ({
    id, revision: 0, name: `factor_${id}`, normalized_name: `factor_${id}`, title: `Factor ${id}`,
    description: '', aliases: [], metadata: {},
    payload: { kind: 'factor', properties: { current: null, desired: null, controllable: false, evidence: [] } },
  }))
  const edges = [
    { source: 'A', destination: 'B' },
    { source: 'B', destination: 'A' },
  ].map(({ source, destination }) => ({
    source, source_kind: 'factor', destination, destination_kind: 'factor', revision: 0,
    description: '', metadata: {},
    payload: { kind: 'contributes', properties: { effect: estimate, lag: null, mechanism: '', evidence: [] } },
  }))
  await page.unroute('**/api/v1/**')
  await mockApi(page, {
    project: { id: 'A', name: 'Feedback model', revision: 0 },
    revision: 4,
    nodes,
    edges,
  })
  await page.goto('/')
  await page.getByRole('button', { name: 'Feedback', exact: true }).click()
  const panel = page.getByLabel('Feedback analysis')
  await expect(panel.getByText('1', { exact: true }).first()).toBeVisible()
  await expect(panel.getByText('A → B → A')).toBeVisible()
  await panel.getByRole('button', { name: /A → B → A/ }).click()
  await expect(panel.getByRole('button', { name: /A → B → A/ })).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByText('Analysis highlights 2 nodes and 2 relationships.')).toBeAttached()
  await expect(page.getByText('feedback', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Impediments' })).toBeEnabled()
  await expect(page.getByRole('button', { name: 'Optimize' })).toBeEnabled()
  await page.screenshot({ path: 'artifacts/workbench-feedback.png', fullPage: true })
})

test('creates and compares finite-horizon scenario candidates', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop workflow assertion')
  const nodes = [
    {
      id: 'A', revision: 0, name: 'reliability', normalized_name: 'reliability', title: 'Reliability',
      description: '', aliases: [], metadata: {},
      payload: { kind: 'outcome', properties: { direction: 'maximize', current: null, desired: null, evidence: [] } },
    },
    {
      id: 'B', revision: 0, name: 'automate', normalized_name: 'automate', title: 'Automate',
      description: '', aliases: [], metadata: {},
      payload: { kind: 'intervention', properties: { costs: [], duration: null, probability_of_success: null, acceptance_criteria: [] } },
    },
  ]
  await page.unroute('**/api/v1/**')
  await mockApi(page, {
    project: { id: 'A', name: 'Optimize model', revision: 0 },
    revision: 2, nodes, edges: [], scenarios: [],
  })
  await page.goto('/')
  await page.getByRole('button', { name: 'Optimize', exact: true }).click()
  await page.getByLabel('Optimize analysis').getByRole('button', { name: 'Create scenario', exact: true }).last().click()
  await page.getByLabel('Title').fill('Reliable delivery')
  await page.getByRole('group', { name: 'Outcome objectives' }).getByText('Reliability', { exact: true }).click()
  await page.getByRole('group', { name: 'Candidate interventions' }).getByText('Automate', { exact: true }).click()
  const scenarioForm = page.getByRole('form', { name: 'Create scenario' })
  await expect.poll(() => scenarioForm.evaluate((form) => (form as HTMLFormElement).checkValidity())).toBe(true)
  await scenarioForm.getByRole('button', { name: 'Create scenario' }).click()
  const panel = page.getByLabel('Optimize analysis')
  await expect(panel.getByText('Automate', { exact: true })).toBeVisible()
  await expect(panel.getByText('0.12')).toBeVisible()
  await expect(panel.getByText('0.004')).toBeVisible()
  await expect(panel.getByText(/No budget, bundle, conflict, synergy, or scalar ranking/)).toBeVisible()
  await panel.getByRole('button', { name: /Automate B converged/ }).click()
  await expect(page.getByText('Analysis highlights 2 nodes and 0 relationships.')).toBeAttached()
  await panel.getByRole('button', { name: /Reliable delivery A · r0 · 12 periods/ }).click()
  const scenarioMenu = page.getByRole('listbox', { name: 'Scenarios' })
  await expect(scenarioMenu.getByRole('option', { name: /Reliable delivery A · r0/ })).toHaveAttribute('aria-selected', 'true')
  await page.keyboard.press('Escape')
  await panel.getByRole('button', { name: 'Edit selected scenario' }).click()
  await expect(page.getByRole('heading', { name: 'Edit scenario' })).toBeVisible()
  await page.getByLabel('Title').fill('Updated delivery')
  await page.getByLabel('Planning horizon in periods').fill('8')
  await page.getByRole('button', { name: 'Save scenario' }).click()
  await expect(panel.getByRole('button', { name: /Updated delivery A · r1 · 8 periods/ })).toBeVisible()
  await expect(panel.getByText('8', { exact: true }).last()).toBeVisible()
  await page.screenshot({ path: 'artifacts/workbench-optimize.png', fullPage: true })
})

test('separates topology and evidence impediment orders', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop workflow assertion')
  const factor = (id: string, title: string, evidence: unknown[]) => ({
    id, revision: 0, name: title.toLocaleLowerCase().replaceAll(' ', '_'), normalized_name: title.toLocaleLowerCase().replaceAll(' ', '_'), title,
    description: '', aliases: [], metadata: {},
    payload: { kind: 'factor', properties: { current: null, desired: null, controllable: id === 'A', evidence } },
  })
  const outcome = (id: string) => ({
    id, revision: 0, name: `outcome_${id}`, normalized_name: `outcome_${id}`, title: `Outcome ${id}`,
    description: '', aliases: [], metadata: {},
    payload: { kind: 'outcome', properties: { direction: 'maximize', current: null, desired: null, evidence: [] } },
  })
  const estimate = { id: 'A', revision: 0, distribution: { type: 'point', value: 0.5 }, provenance: [] }
  const edge = (source: string, destination: string, evidence: string[]) => ({
    source, source_kind: 'factor', destination, destination_kind: 'outcome', revision: 0,
    description: '', metadata: {},
    payload: { kind: 'contributes', properties: { effect: estimate, lag: null, mechanism: '', evidence } },
  })
  await page.unroute('**/api/v1/**')
  await mockApi(page, {
    project: { id: 'A', name: 'Impediment model', revision: 0 }, revision: 5,
    nodes: [
      factor('A', 'Wide reach', []),
      factor('B', 'Documented', [{ id: 0, revision: 0, summary: 'Observed', source: null }]),
      outcome('C'), outcome('D'),
    ],
    edges: [edge('A', 'C', []), edge('A', 'D', []), edge('B', 'C', ['ADR-1'])],
  })
  await page.goto('/')
  await page.getByRole('button', { name: 'Impediments', exact: true }).click()
  const panel = page.getByLabel('Impediments analysis')
  await expect(panel.locator('.impediment-title strong').first()).toHaveText('Wide reach')
  await expect(panel.getByText('2 path edges lack typed evidence.')).toBeVisible()
  await panel.getByRole('button', { name: /Evidence/ }).click()
  await expect(panel.locator('.impediment-title strong').first()).toHaveText('Documented')
  await panel.locator('.impediment-list > li > button').first().click()
  await expect(page.getByText('Analysis highlights 2 nodes and 1 relationships.')).toBeAttached()
  await expect(panel.getByText(/Neither is a causal confidence score/)).toBeVisible()
  await page.screenshot({ path: 'artifacts/workbench-impediments.png', fullPage: true })
})

test('keeps project and graph controls usable on mobile', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'mobile', 'mobile-only layout assertion')
  await page.goto('/')
  await page.getByRole('button', { name: 'Create project' }).click()
  await page.getByLabel('Project name').fill('Mobile model')
  await page.getByRole('button', { name: 'Create project' }).last().click()
  await page.getByRole('button', { name: 'Add node' }).last().click()
  await page.getByLabel('Title').fill('Customer trust')
  await page.getByText('outcome', { exact: true }).click()
  await page.getByRole('button', { name: 'Add node' }).last().click()

  await expect(page.getByLabel('Project', { exact: true })).toBeVisible()
  await expect(page.getByLabel('Search graph')).toBeVisible()
  await expect(page.getByTestId('graph-surface')).toBeVisible()
  await expectCanvasPainted(page)
  await page.getByRole('button', { name: /Customer trust/ }).click()
  await page.getByRole('button', { name: 'Estimate' }).click()
  await page.getByLabel('Distribution', { exact: true }).selectOption('beta')
  await page.getByRole('button', { name: 'Explain Alpha' }).click()
  await expect(page.getByText(/Increasing alpha relative to beta/)).toBeVisible()
  const previewBox = await page.getByLabel('Beta distribution preview').boundingBox()
  expect(previewBox).not.toBeNull()
  expect(previewBox!.x).toBeGreaterThanOrEqual(0)
  expect(previewBox!.x + previewBox!.width).toBeLessThanOrEqual(page.viewportSize()!.width)
  await page.screenshot({ path: 'artifacts/workbench-mobile.png', fullPage: true })
})

test('renders a bounded 100-node model without a blank canvas', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop performance assertion')
  const nodes = Array.from({ length: 100 }, (_, index) => ({
    id: `N${index}`,
    revision: 0,
    name: `factor_${index}`,
    normalized_name: `factor_${index}`,
    title: `Factor ${index}`,
    description: '',
    aliases: [],
    metadata: {},
    payload: {
      kind: 'factor',
      properties: { current: null, desired: null, controllable: false, evidence: [] },
    },
  }))
  await page.unroute('**/api/v1/**')
  await mockApi(page, {
    project: { id: 'A', name: 'Performance fixture', revision: 0 },
    revision: 0,
    nodes,
    edges: [],
  })
  const started = Date.now()
  await page.goto('/')
  await expect(page.getByText('100 nodes')).toBeVisible()
  await expectCanvasPainted(page)
  expect(Date.now() - started).toBeLessThan(5_000)
})
