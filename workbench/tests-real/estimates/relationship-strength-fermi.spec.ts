import { expect, test } from '@playwright/test'
import { addNode, addRelationship, createProject } from '../support/workbench'

test('derives signed relationship strength from Squiggle source', async ({ page }) => {
  await createProject(page, 'Squiggle relationship strength')
  await addNode(page, 'Review quality')
  await addNode(page, 'Delivery confidence')
  await addRelationship(page, 'contributes', 'A', 'B', { effect: 0.4 })
  await page.getByRole('button', { name: /Review quality/ }).click()
  await page.getByRole('button', { name: 'Edit contributes relationship A to B' }).click()
  await page.getByRole('button', { name: 'Edit relationship effect estimate' }).click()
  await page.getByLabel('Squiggle source').fill('mechanism = beta(8, 2)\nevidence = beta(7, 3)\nmechanism * evidence * 2 - 1')
  await expect(page.getByText(/effective samples/)).toBeVisible()
  await page.getByRole('button', { name: 'Replace estimate' }).click()
  await expect(page.getByText(/Empirical/)).toBeVisible()
  await expect(page.getByText(/Squiggle · mechanism = beta/)).toBeVisible()
})