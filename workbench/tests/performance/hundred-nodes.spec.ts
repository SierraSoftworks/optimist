import { test, expect, expectCanvasPainted, mockApi } from '../support/mock-api'

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
  expect(await page.evaluate(() => document.documentElement.scrollHeight)).toBeLessThanOrEqual(page.viewportSize()!.height)
  await expectCanvasPainted(page)
  await expect(page.getByRole('button', { name: 'Cluster by kind' })).toHaveAttribute('aria-pressed', 'true')
  await expect(page.locator('.detail-indicator')).toHaveText('overview')
  await page.locator('.canvas-panel').screenshot({ path: 'artifacts/graph-semantic-overview.png' })
  await page.getByRole('button', { name: 'Hierarchy layout' }).click()
  await page.getByRole('button', { name: 'Cluster by kind' }).click()
  await expect(page.getByRole('button', { name: 'Cluster by kind' })).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByLabel('Node kind clusters')).toContainText('Factors 100')
  await expectCanvasPainted(page)
  for (let step = 0; step < 8; step += 1) {
    await page.getByRole('button', { name: 'Zoom in' }).click()
  }
  await expect(page.locator('.detail-indicator')).toHaveText('detail')
  expect(Date.now() - started).toBeLessThan(5_000)
})
