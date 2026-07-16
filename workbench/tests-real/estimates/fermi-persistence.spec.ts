import { expect, test } from '@playwright/test'
import { addNode, createProject } from '../support/workbench'

test('persists, reviews, reopens, and replaces a Fermi estimate source', async ({ page }) => {
  await createProject(page, 'Persisted Fermi estimate')
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
  await page.getByRole('button', { name: 'Use Fermi equation' }).click()
  await page.getByRole('button', { name: 'Set estimate' }).click()

  await expect(page.getByText(/Current Fermi/)).toBeVisible()
  await expect(page.getByText(/adoption \* completion · 2 variables · converged/)).toBeVisible()
  await page.getByRole('button', { name: 'Estimate', exact: true }).click()
  await expect(page.getByText('Stored effective result')).toBeVisible()
  await expect(page.getByLabel('Fermi equation')).toHaveValue('adoption * completion')
  await expect(page.getByLabel('Variable 1 name')).toHaveValue('adoption')
  await expect(page.getByLabel('Variable 2 name')).toHaveValue('completion')

  await page.getByRole('button', { name: 'Distribution' }).click()
  await page.getByRole('spinbutton', { name: 'Alpha', exact: true }).fill('8')
  await page.getByRole('spinbutton', { name: 'Beta', exact: true }).fill('2')
  await page.getByRole('button', { name: 'Replace estimate' }).click()
  await expect(page.getByText('Beta · α 8, β 2')).toBeVisible()
  await expect(page.getByText(/Current Fermi/)).toHaveCount(0)
})