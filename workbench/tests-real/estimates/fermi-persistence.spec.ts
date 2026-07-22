import { expect, test } from '@playwright/test'
import { addNode, createProject } from '../support/workbench'

test('persists, reviews, reopens, and replaces a Squiggle estimate source', async ({ page }) => {
  await createProject(page, 'Persisted Squiggle estimate')
  await addNode(page, 'Delivery confidence')
  await page.getByRole('button', { name: /Delivery confidence/ }).click()
  await page.getByRole('button', { name: 'Estimate', exact: true }).click()
  await page.getByLabel('Squiggle source').fill('adoption = beta(8, 2)\ncompletion = beta(7, 3)\nadoption * completion')
  await expect(page.getByText(/effective samples/)).toBeVisible()
  await page.getByRole('button', { name: 'Replace estimate' }).click()

  await expect(page.getByText(/Current model/)).toBeVisible()
  await expect(page.getByText(/adoption = beta/)).toBeVisible()
  await page.getByRole('button', { name: 'Estimate', exact: true }).click()
  await expect(page.getByLabel('Squiggle source')).toHaveValue('adoption = beta(8, 2)\ncompletion = beta(7, 3)\nadoption * completion')

  await page.getByLabel('Squiggle source').fill('beta(9, 1)')
  await expect(page.getByText(/effective samples/)).toBeVisible()
  await page.getByRole('button', { name: 'Replace estimate' }).click()
  await expect(page.getByText(/beta\(9, 1\) · Beta/)).toBeVisible()
})