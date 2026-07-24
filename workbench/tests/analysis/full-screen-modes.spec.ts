import { expect, mockApi, test, type FixtureState } from '../support/mock-api'

const config = {
  seed: 42, minimum_samples: 10, maximum_samples: 20,
  absolute_tolerance: 0.01, relative_tolerance: 0.01,
}

const state: FixtureState = {
  project: { id: 'A', name: 'Analysis workspaces', revision: 0 },
  revision: 0,
  nodes: [
    {
      id: 'A', revision: 0, name: 'reliability', normalized_name: 'reliability', title: 'Reliability',
      description: '', aliases: [], metadata: {},
      native_state: {
        quantity: { unit: 'state', dimension: {}, aggregation: null, support: { type: 'bounded', lower: 0, upper: 1 } },
        current: null, forecast: null,
      },
      payload: { kind: 'outcome', properties: { direction: 'maximize', evidence: [] } },
    },
    {
      id: 'B', revision: 0, name: 'automation', normalized_name: 'automation', title: 'Automation',
      description: '', aliases: [], metadata: {},
      payload: { kind: 'intervention', properties: { costs: [], duration: null, probability_of_success: null, acceptance_criteria: [] } },
    },
  ],
  edges: [],
  scenarios: [{
    id: 'A', revision: 0, name: 'plan', title: 'Plan', rationale: '',
    objectives: [{ outcome_id: 'A', direction: 'maximize', importance: 1 }],
    planning_horizon: 4, budgets: [], candidate_interventions: ['B'], monte_carlo: config,
  }],
}

test('uses full-screen workspaces for impediments and optimize', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop mode navigation workflow')
  await page.unroute('**/api/v1/**')
  await mockApi(page, structuredClone(state))
  await page.goto('/')
  await expect(page.locator('.canvas-panel')).toBeVisible()
  await expect(page.getByRole('complementary', { name: 'Graph navigator' })).toBeVisible()

  await page.getByRole('button', { name: 'Impediments' }).click()
  await expect(page.getByRole('main', { name: 'Impediments analysis' })).toBeVisible()
  await expect(page.locator('.canvas-panel')).toHaveCount(0)
  await expect(page.getByRole('complementary', { name: 'Graph navigator' })).toHaveCount(0)

  await page.getByRole('button', { name: 'Optimize' }).click()
  await expect(page.getByRole('main', { name: 'Optimize analysis' })).toBeVisible()
  await expect(page.locator('.canvas-panel')).toHaveCount(0)
  await expect(page.getByRole('complementary', { name: 'Graph navigator' })).toHaveCount(0)
})
