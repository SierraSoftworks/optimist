import { expect, test } from '@playwright/test'
import { addNode, addRelationship, createProject, setPointEstimate } from '../support/workbench'

test('creates, selects, edits, and analyzes a scenario', async ({ page }) => {
  await createProject(page, 'Optimize analysis')
  await addNode(page, 'Pair review', 'intervention')
  await addNode(page, 'Review throughput')
  await setPointEstimate(page, 0.4)
  await addNode(page, 'Delivery reliability', 'outcome')
  await setPointEstimate(page, 0.5)
  await addRelationship(page, 'changes', 'A', 'B', { effect: 0.25 })
  await addRelationship(page, 'contributes', 'B', 'C', { effect: 0.5 })

  await page.getByRole('button', { name: 'Optimize', exact: true }).click()
  await page.getByLabel('Optimize analysis').getByRole('button', { name: 'Create scenario', exact: true }).last().click()
  await page.getByLabel('Title').fill('Pair review impact')
  await page.getByRole('group', { name: 'Outcome objectives' }).getByText('Delivery reliability', { exact: true }).click()
  await page.getByRole('group', { name: 'Candidate interventions' }).getByText('Pair review', { exact: true }).click()
  await page.getByText('Sampling controls').click()
  await page.getByLabel('Minimum samples').fill('10')
  await page.getByLabel('Maximum samples').fill('100')
  await page.getByRole('form', { name: 'Create scenario' }).getByRole('button', { name: 'Create scenario' }).click()

  const panel = page.getByLabel('Optimize analysis')
  await expect(panel.getByText('Pair review', { exact: true })).toBeVisible()
  await expect(panel.getByText('10 / 10')).toBeVisible()
  await panel.getByRole('button', { name: /Pair review impact A · r0 · 12 periods/ }).click()
  const menu = page.getByRole('listbox', { name: 'Scenarios' })
  await expect(menu.getByRole('option', { name: /Pair review impact A · r0/ })).toHaveAttribute('aria-selected', 'true')
  await expect(page.locator('body > .scenario-menu')).toBeVisible()
  await page.keyboard.press('Escape')

  await panel.getByRole('button', { name: 'Edit selected scenario' }).click()
  await page.getByLabel('Title').fill('Updated pair review')
  await page.getByLabel('Planning horizon in periods').fill('8')
  await page.getByRole('button', { name: 'Save scenario' }).click()
  await expect(panel.getByRole('button', { name: /Updated pair review A · r1 · 8 periods/ })).toBeVisible()
  await expect(panel.getByText('8', { exact: true }).last()).toBeVisible()
})
