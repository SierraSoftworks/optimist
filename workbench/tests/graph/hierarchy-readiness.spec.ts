import { expect, mockApi, test, type FixtureState } from '../support/mock-api'

const estimate = (id: string, value: number) => ({
  id,
  revision: 0,
  distribution: { type: 'point', value },
  source: { type: 'distribution' },
})

test('orders causal hierarchy and exposes focused relationship metadata', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop hierarchy assertion')
  const state: FixtureState = {
    project: { id: 'A', name: 'Hierarchy review', revision: 0 },
    revision: 0,
    nodes: [
      {
        id: 'A', revision: 0, name: 'pair_review', normalized_name: 'pair_review', title: 'Pair review', description: '', aliases: [], metadata: {},
        payload: { kind: 'intervention', properties: { costs: [], duration: estimate('A', 2), probability_of_success: estimate('B', 0.8), acceptance_criteria: [] } },
      },
      {
        id: 'B', revision: 0, name: 'review_flow', normalized_name: 'review_flow', title: 'Review flow', description: '', aliases: [], metadata: {},
        payload: { kind: 'factor', properties: { current: estimate('A', 0.45), desired: null, controllable: true, evidence: [] } },
      },
      {
        id: 'C', revision: 0, name: 'delivery', normalized_name: 'delivery', title: 'Reliable delivery', description: '', aliases: [], metadata: {},
        payload: { kind: 'outcome', properties: { direction: 'maximize', current: null, desired: null, evidence: [] } },
      },
    ],
    edges: [
      {
        source: 'A', source_kind: 'intervention', destination: 'B', destination_kind: 'factor', revision: 0, description: '', metadata: {},
        payload: { kind: 'changes', properties: { effect: estimate('A', 0.35), lag: null, mechanism: 'Automates checks', evidence: [] } },
      },
      {
        source: 'B', source_kind: 'factor', destination: 'C', destination_kind: 'outcome', revision: 0, description: '', metadata: {},
        payload: { kind: 'contributes', properties: { effect: estimate('A', 0.6), lag: null, mechanism: 'Shortens review', evidence: [] } },
      },
    ],
  }
  await page.unroute('**/api/v1/**')
  await mockApi(page, state)
  await page.goto('/')
  await page.getByRole('button', { name: /Review flow/ }).click()

  await expect(page.getByText('1 need setup')).toBeVisible()
  await page.getByRole('button', { name: 'Needs setup 1' }).click()
  await expect(page.getByText('1 nodes')).toBeVisible()
  await expect(page.getByRole('button', { name: /Reliable delivery/ })).toBeVisible()
  await expect(page.getByRole('button', { name: /Pair review/ })).toHaveCount(0)
  await page.getByRole('button', { name: 'Needs setup 1' }).click()
  await page.getByRole('button', { name: /Review flow/ }).click()
  await page.getByRole('button', { name: 'Cluster by kind' }).click()
  const clusters = page.getByLabel('Node kind clusters')
  await expect(clusters).toContainText('Actions 1')
  await expect(clusters).toContainText('Factors 1')
  await expect(clusters).toContainText('Objectives 1')
  await page.screenshot({ path: 'artifacts/graph-kind-clusters.png', fullPage: true })
  await page.getByRole('button', { name: 'Hierarchy layout' }).click()
  const focused = page.getByRole('region', { name: 'Focused relationships' })
  await expect(focused.getByText('changes · mean effect +0.35')).toBeVisible()
  await expect(focused.getByText('contributes · mean effect +0.60')).toBeVisible()

  const centers = await page.locator('.graph-canvas canvas[data-id="layer2-node"]').evaluate((element) => {
    const canvas = element as HTMLCanvasElement
    const context = canvas.getContext('2d')!
    const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data
    const colors = {
      intervention: [232, 152, 115],
      factor: [142, 182, 224],
      outcome: [245, 184, 63],
    } as const
    const sums = Object.fromEntries(Object.keys(colors).map((kind) => [kind, { y: 0, count: 0 }])) as Record<string, { y: number; count: number }>
    for (let y = 0; y < canvas.height; y += 1) {
      for (let x = 0; x < canvas.width; x += 1) {
        const offset = (y * canvas.width + x) * 4
        for (const [kind, color] of Object.entries(colors)) {
          if (Math.abs(pixels[offset]! - color[0]) < 5 && Math.abs(pixels[offset + 1]! - color[1]) < 5 && Math.abs(pixels[offset + 2]! - color[2]) < 5 && pixels[offset + 3]! > 200) {
            sums[kind]!.y += y
            sums[kind]!.count += 1
          }
        }
      }
    }
    return Object.fromEntries(Object.entries(sums).map(([kind, value]) => [kind, value.y / value.count]))
  })
  expect(centers.intervention).toBeLessThan(centers.factor!)
  expect(centers.factor).toBeLessThan(centers.outcome!)

  await focused.getByRole('button', { name: 'Edit focused relationship B contributes C' }).click()
  await expect(page.getByRole('heading', { name: 'Edit relationship' })).toBeVisible()
  await page.getByRole('button', { name: 'Close' }).click()
  await page.screenshot({ path: 'artifacts/graph-hierarchy-readiness.png', fullPage: true })
})