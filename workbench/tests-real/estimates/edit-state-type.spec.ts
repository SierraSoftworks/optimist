import { expect, test } from '@playwright/test'
import { addNode, createProject } from '../support/workbench'

const source = `changesPerMonth :: change/month = normal({ p10: 50, p90: 500 })
changesPerMonth`

test('edits a factor state type and revalidates its Squiggle estimate', async ({ page }) => {
  await createProject(page, 'Editable state type')
  await addNode(page, 'Change frequency', 'factor')

  await page.getByRole('button', { name: 'State type' }).click()
  let typeDialog = page.getByRole('form', { name: 'Edit state type' })
  await typeDialog.getByLabel('Unit').fill('changes/month')
  await typeDialog.getByLabel('Aggregation').fill('total monthly')
  await typeDialog.getByLabel('Support').selectOption('non_negative')
  await typeDialog.getByRole('button', { name: 'Save state type' }).click()
  await expect(typeDialog).toBeHidden()

  await page.getByRole('button', { name: 'Estimate', exact: true }).click()
  await expect(page.getByText(/Validated/)).toBeVisible()
  const sourceEditor = page.getByLabel('Squiggle source')
  await sourceEditor.fill(source)
  await expect(sourceEditor).toHaveValue(source)
  await expect(page.getByText('Normal is incompatible with this state type')).toBeVisible()
  await expect(page.getByRole('button', { name: 'Replace estimate' })).toBeDisabled()
  await page.getByRole('button', { name: 'Close' }).click()

  await page.getByRole('button', { name: 'State type' }).click()
  typeDialog = page.getByRole('form', { name: 'Edit state type' })
  await typeDialog.getByLabel('Support').selectOption('real')
  await typeDialog.getByRole('button', { name: 'Save state type' }).click()
  await expect(typeDialog).toBeHidden()

  await page.getByRole('button', { name: 'Estimate', exact: true }).click()
  await expect(page.getByText(/Validated/)).toBeVisible()
  await page.getByLabel('Squiggle source').fill(source)
  await expect(page.getByText(/Validated/)).toBeVisible()
  await page.getByRole('button', { name: 'Replace estimate' }).click()
  await expect(page.getByLabel('Squiggle source')).toBeHidden()
  await expect(page.getByRole('alert')).toBeHidden()
})