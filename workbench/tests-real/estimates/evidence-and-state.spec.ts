import { expect, test } from '@playwright/test'
import { addNode, createProject } from '../support/workbench'

test('edits node metadata, evidence, and state estimates', async ({ page }) => {
  await createProject(page, 'Evidence and state')
  await addNode(page, 'Fast feedback')
  await page.getByRole('button', { name: /Fast feedback/ }).click()
  await page.getByRole('button', { name: 'Details' }).click()
  await page.getByLabel('Title').fill('Rapid feedback')
  await page.getByLabel('Description').fill('Short feedback loops.')
  await page.getByLabel('Metadata').fill('{"owner":"platform"}')
  await page.getByRole('button', { name: 'Save details' }).click()

  await page.getByRole('button', { name: 'Add evidence' }).click()
  await page.getByLabel('Summary').fill('Queueing observed during review')
  await page.getByLabel('Source').fill('Delivery dashboard')
  await page.getByRole('form', { name: 'Add evidence' }).getByRole('button', { name: 'Add evidence' }).click()
  await page.getByRole('button', { name: /Edit evidence/ }).click()
  await page.getByLabel('Summary').fill('Queueing confirmed during review')
  await page.getByRole('button', { name: 'Save evidence' }).click()
  await expect(page.getByText(/Delivery dashboard · r1/)).toBeVisible()

  await page.getByRole('button', { name: 'Estimate' }).click()
  await page.getByLabel('Value on [0, 1]').fill('0.65')
  await page.getByLabel('Provenance').fill('Weekly review')
  await page.getByRole('button', { name: 'Set estimate' }).click()
  await expect(page.getByText('Point · 0.65')).toBeVisible()
  await expect(page.getByText('Weekly review')).toBeVisible()
})
