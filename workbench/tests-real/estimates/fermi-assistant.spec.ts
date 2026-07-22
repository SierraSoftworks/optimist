import { expect, test } from '@playwright/test'
import { addNode, createProject } from '../support/workbench'

test('derives a normalized state estimate from rich Squiggle source', async ({ page }) => {
  await createProject(page, 'Squiggle state estimate')
  await addNode(page, 'Delivery confidence')
  await page.getByRole('button', { name: /Delivery confidence/ }).click()
  await page.getByRole('button', { name: 'Estimate', exact: true }).click()
  await page.getByLabel('Squiggle source').fill('mixture([beta(8, 2), beta(3, 7)], [0.8, 0.2])')
  await expect(page.getByText(/effective samples/)).toBeVisible()
  await expect(page.getByText('90% interval')).toBeVisible()
  await page.getByRole('button', { name: 'Replace estimate' }).click()

  await expect(page.getByText(/Empirical/)).toBeVisible()
  await expect(page.getByText(/mixture\(\[beta/)).toBeVisible()
})