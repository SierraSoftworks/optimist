import { test, expect, mockApi } from '../support/mock-api'

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
  await expect(panel.getByRole('cell', { name: '0.12' })).toBeVisible()
  await expect(panel.getByRole('cell', { name: '0.004' })).toBeVisible()
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
