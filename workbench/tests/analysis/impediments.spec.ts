import { test, expect, mockApi } from '../support/mock-api'

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
