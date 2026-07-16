import { expect, test } from '@playwright/test'
import { addNode, addRelationship, createProject } from '../support/workbench'

test('derives signed relationship strength from a Fermi decomposition', async ({ page }) => {
  await createProject(page, 'Fermi relationship strength')
  await addNode(page, 'Review quality')
  await addNode(page, 'Delivery confidence')
  await addRelationship(page, 'contributes', 'A', 'B', { effect: 0.4 })
  await page.getByRole('button', { name: /Review quality/ }).click()
  await page.getByRole('button', { name: 'Edit contributes relationship A to B' }).click()
  await page.getByRole('button', { name: 'Edit relationship effect estimate' }).click()
  await page.getByRole('button', { name: /Fermi decomposition/ }).click()
  await page.getByLabel('Variable 1 name').fill('mechanism_strength')
  await page.getByLabel('Variable 2 name').fill('evidence_reliability')
  await page.getByLabel('Fermi equation').fill('mechanism_strength * evidence_reliability')
  await page.getByRole('button', { name: 'Assess equation' }).click()
  await expect(page.getByText(/samples · converged/)).toBeVisible()
  await page.getByRole('button', { name: 'Use suggested distribution' }).click()
  await expect(page.getByLabel('Distribution', { exact: true })).toHaveValue('scaled_beta')
  await page.getByRole('button', { name: 'Replace estimate' }).click()
  await expect(page.getByText(/Scaled Beta · \[-1, 1\]/)).toBeVisible()
})