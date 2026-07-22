import { expect, test } from '@playwright/test'
import { addNode, createProject, saveSquiggleEstimate } from '../support/workbench'

test('authors an intervention metric shift as a Squiggle response', async ({ page }) => {
  await createProject(page, 'Intervention metric response')
  await addNode(page, 'Lead time', 'metric', 'days')
  await page.getByRole('button', { name: 'Estimate', exact: true }).click()
  await saveSquiggleEstimate(page, 'pointMass(10)', 'Set estimate')
  await addNode(page, 'Automate review', 'intervention')

  await page.getByRole('button', { name: 'Relationship', exact: true }).click()
  const relationship = page.getByRole('form', { name: 'Add relationship' })
  await relationship.getByRole('combobox').first().selectOption('changes')
  await page.getByLabel('Source').selectOption('B')
  await page.getByLabel('Destination').selectOption('A')
  await expect(relationship.getByText('Counterfactual response')).toBeVisible()
  await expect(relationship.getByLabel('Squiggle editor and distribution viewer')).toBeVisible()
  await page.getByLabel('Squiggle source').fill('normal(-2, 0.5)')
  await expect(relationship.getByText(/effective samples/)).toBeVisible()
  await relationship.getByRole('button', { name: 'Add relationship' }).click()

  await expect(page.getByText('1 relationships')).toBeVisible()
  await expect(page.getByText('changes · mean response -2.00')).toBeVisible()
  await page.getByRole('button', { name: 'Edit focused relationship B changes A' }).click()
  await expect(page.getByText('1 1 → Empirical · 2,048 samples day')).toBeVisible()
  await expect(page.getByText(/Squiggle · normal\(-2, 0.5\)/)).toBeVisible()
})
