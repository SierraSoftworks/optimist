import { expect, test } from '@playwright/test'
import { addNode, createProject } from '../support/workbench'

test('models a duration with direct Squiggle code and unit verification', async ({ page }) => {
  await createProject(page, 'Piano tuning estimate')
  await addNode(page, 'Piano tuning demand', 'intervention')
  await page.getByRole('button', { name: /Piano tuning demand/ }).click()
  await page.getByRole('button', { name: 'Edit duration estimate' }).click()
  await page.getByLabel('Squiggle source').fill('result :: wrong_unit = gamma(4, 3)\nresult')
  await expect(page.getByText(/declared unit|does not match/)).toBeVisible()
  await page.getByLabel('Squiggle source').fill('result :: duration = gamma(4, 3)\nresult')
  await expect(page.getByText(/effective samples/)).toBeVisible()
  await page.getByRole('button', { name: 'Replace estimate' }).click()
  await expect(page.getByText(/Squiggle · result :: duration/)).toBeVisible()
})
