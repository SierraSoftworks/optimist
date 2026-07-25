import { test, expect, mockApi } from '../support/mock-api'
import { causalEdge, factorNode, interventionNode, outcomeNode } from '../support/fixtures'

test('ranks intervention readiness with execution plans and blockers', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop workflow assertion')
  await page.unroute('**/api/v1/**')
  await mockApi(page, {
    project: { id: 'A', name: 'Impediment model', revision: 0 },
    revision: 5,
    nodes: [
      interventionNode('A', 'Wide reach', { duration: 3, probability: 0.8 }),
      interventionNode('B', 'Documented', { duration: 1, probability: 0.9 }),
      factorNode('C', 'Review flow', { controllable: true, current: 0.4 }),
      outcomeNode('D', 'Reliable delivery', { current: 0.5 }),
    ],
    edges: [
      causalEdge('A', 'intervention', 'C', 'factor', { kind: 'changes', response: 0.3 }),
      causalEdge('C', 'factor', 'D', 'outcome', { response: 0.6 }),
    ],
  })
  await page.goto('/')
  await page.getByRole('button', { name: 'Impediments', exact: true }).click()

  const panel = page.getByLabel('Impediments analysis')
  await expect(panel).toBeVisible()

  // Candidates are ranked in server order, and the priority badge must reflect it.
  const cards = panel.locator('.readiness-card')
  await expect(cards).toHaveCount(2)
  await expect(cards.first().getByRole('heading', { name: 'Wide reach' })).toBeVisible()
  await expect(cards.first().locator('.priority')).toHaveText('1')
  await expect(cards.nth(1).getByRole('heading', { name: 'Documented' })).toBeVisible()

  // Nothing blocks either candidate, so both must read as executable.
  await expect(cards.first().locator('.readiness-badge')).toHaveText(/Executable/)
  await expect(cards.nth(1).locator('.readiness-badge')).toHaveText(/Executable/)

  await page.screenshot({ path: 'artifacts/workbench-impediments.png', fullPage: true })
})
