import { test, expect, mockApi } from '../support/mock-api'

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
