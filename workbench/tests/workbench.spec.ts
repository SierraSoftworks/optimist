import { expect, test, type Page } from '@playwright/test'

interface FixtureState {
  revision: number
  project: { id: string; name: string; revision: number } | null
  projects?: Array<{ id: string; name: string; revision: number }>
  nodes: Array<Record<string, unknown>>
  edges: Array<Record<string, unknown>>
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
    if (url.pathname === '/api/v1/projects/A/commands' && request.method() === 'POST') {
      const command = JSON.parse(request.postData()!)
      const input = command.command.payload
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
