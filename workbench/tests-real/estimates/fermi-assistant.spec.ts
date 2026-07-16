import { expect, test } from '@playwright/test'
import { addNode, createProject } from '../support/workbench'

test('derives a normalized state estimate from a Fermi decomposition', async ({ page }) => {
  await createProject(page, 'Fermi state estimate')
  await addNode(page, 'Delivery confidence')
  await page.getByRole('button', { name: /Delivery confidence/ }).click()
  await page.getByRole('button', { name: 'Estimate', exact: true }).click()
  await page.getByRole('button', { name: 'Fermi equation' }).click()
  await page.getByRole('button', { name: /Fermi decomposition/ }).click()
  await page.getByLabel('Variable 1 name').fill('adoption')
  await page.getByLabel('Variable 2 name').fill('completion')
  await page.getByLabel('Fermi equation').fill('adoption * completion')
  await page.getByRole('button', { name: 'Assess equation' }).click()

  await expect(page.getByText(/samples · converged/)).toBeVisible()
  await expect(page.locator('.fermi-result').getByText('Derived unit1', { exact: true })).toBeVisible()
  await expect(page.getByText(/preserves sampled mean and variance/)).toBeVisible()
  await page.getByRole('button', { name: 'Use Fermi equation' }).click()
  await page.getByRole('button', { name: 'Set estimate' }).click()

  await expect(page.getByText(/Beta · α/)).toBeVisible()
  await expect(page.getByText(/adoption \* completion · 2 variables · converged/)).toBeVisible()
})