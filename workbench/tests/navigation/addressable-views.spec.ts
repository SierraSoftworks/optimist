import { test, expect, mockApi } from '../support/mock-api'
import { factorNode } from '../support/fixtures'

/**
 * The project and the view live in the address bar, so a reload, a bookmark, or
 * a pasted link lands where it left off instead of resetting to the first
 * project's explore view.
 */
test('restores the project and view from the address bar', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop workflow assertion')
  await page.unroute('**/api/v1/**')
  await mockApi(page, {
    project: { id: 'A', name: 'Routed model', revision: 0 },
    revision: 4,
    nodes: [factorNode('A', 'Factor A', { current: 0.5 })],
    edges: [],
  })

  await page.goto('/projects/A/impediments')
  await expect(page.getByRole('button', { name: 'Impediments' })).toHaveAttribute('aria-pressed', 'true')

  await page.reload()
  await expect(page.getByRole('button', { name: 'Impediments' })).toHaveAttribute('aria-pressed', 'true')
})

test('records each view change in history and steps back through them', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop workflow assertion')
  await page.unroute('**/api/v1/**')
  await mockApi(page, {
    project: { id: 'A', name: 'Routed model', revision: 0 },
    revision: 4,
    nodes: [factorNode('A', 'Factor A', { current: 0.5 })],
    edges: [],
  })

  // Bootstrapping from the bare workbench replaces rather than pushes, so the
  // first entry is already the canonical address.
  await page.goto('/')
  await expect(page).toHaveURL('/projects/A/explore')

  await page.getByRole('button', { name: 'Feedback', exact: true }).click()
  await expect(page).toHaveURL('/projects/A/feedback')

  await page.goBack()
  await expect(page).toHaveURL('/projects/A/explore')
  await expect(page.getByRole('button', { name: 'Explore' })).toHaveAttribute('aria-pressed', 'true')
})

test('lands a link naming an unknown project on one this server has', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop workflow assertion')
  await page.unroute('**/api/v1/**')
  await mockApi(page, {
    project: { id: 'A', name: 'Routed model', revision: 0 },
    revision: 4,
    nodes: [factorNode('A', 'Factor A', { current: 0.5 })],
    edges: [],
  })

  await page.goto('/projects/ZZZ/feedback')
  await expect(page).toHaveURL('/projects/A/feedback')
  await expect(page.getByRole('button', { name: 'Feedback', exact: true })).toHaveAttribute('aria-pressed', 'true')
})
