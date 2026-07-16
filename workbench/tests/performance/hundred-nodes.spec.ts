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
  await expectCanvasPainted(page)
  expect(Date.now() - started).toBeLessThan(5_000)
})
