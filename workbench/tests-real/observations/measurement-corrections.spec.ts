import { expect, test } from '@playwright/test'
import { addNode, addRelationship, createProject } from '../support/workbench'

test('appends and corrects measurement observations', async ({ page }) => {
  await createProject(page, 'Measurement corrections')
  await addNode(page, 'Cycle time', 'metric', 'days')
  await addNode(page, 'Flow')
  await addRelationship(page, 'measures', 'A', 'B')
  await page.getByRole('button', { name: /Cycle time/ }).click()
  await page.getByRole('button', { name: 'Add observation for B' }).click()
  await page.getByLabel('Value').fill('4.2')
  await page.getByLabel('Source').fill('Delivery dashboard')
  await page.getByLabel('Include known measurement error').check()
  await page.getByLabel('Standard deviation').fill('0.2')
  await page.getByRole('button', { name: 'Add observation', exact: true }).click()
  await expect(page.getByText('4.2 days')).toBeVisible()

  await page.getByRole('button', { name: 'Correct observation 0 for B' }).click()
  await page.getByLabel('Corrected value').fill('3.9')
  await page.getByRole('button', { name: 'Append correction' }).click()
  await expect(page.getByText('3.9 days')).toBeVisible()
  await expect(page.getByText('Superseded by #1')).toBeVisible()
  await expect(page.getByText('Correction of #0')).toBeVisible()
})
