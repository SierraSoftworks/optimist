import { expect, test } from '@playwright/test'
import { addNode, createProject } from '../support/workbench'

test('calibrates metric readings and explicitly adopts one as factor state', async ({ page }) => {
  await createProject(page, 'Metric calibration')
  await addNode(page, 'Cycle time', 'metric', 'days')
  await addNode(page, 'Flow')

  await page.getByRole('button', { name: 'Relationship', exact: true }).click()
  const relationship = page.getByRole('form', { name: 'Add relationship' })
  await relationship.getByRole('combobox').first().selectOption('measures')
  await page.getByLabel('Source').selectOption('A')
  await page.getByLabel('Destination').selectOption('B')
  await page.getByLabel('Measurement polarity').selectOption('lower_is_better')
  await relationship.getByRole('button', { name: 'Add relationship' }).click()

  await page.getByRole('button', { name: /Cycle time/ }).click()
  await page.getByRole('button', { name: 'Edit measures relationship A to B' }).click()
  const calibration = page.locator('.calibration-editor')
  await calibration.getByLabel('Calibrated').check()
  await calibration.getByLabel('Reading at state 0').fill('20')
  await calibration.getByLabel('Reading at state 1').fill('5')
  await expect(calibration.getByText('20 metric units → state 0 · 5 metric units → state 1')).toBeVisible()
  await calibration.getByRole('button', { name: 'Save calibration' }).click()
  await page.getByRole('button', { name: 'Close' }).click()

  await page.getByRole('button', { name: 'Add observation for B' }).click()
  await page.getByLabel('Value').fill('12.5')
  await page.getByLabel('Source').fill('Delivery dashboard')
  await page.getByRole('button', { name: 'Add observation', exact: true }).click()
  await expect(page.getByText('Normalized factor state 0.500')).toBeVisible()

  await page.getByRole('button', { name: /Flow/ }).click()
  await page.getByRole('button', { name: 'Estimate', exact: true }).click()
  await expect(page.getByText('12.5 days → 0.500')).toBeVisible()
  await page.getByRole('button', { name: 'Use reading' }).click()
  await expect(page.getByLabel('Value on [0, 1]')).toHaveValue('0.5')
  await expect(page.getByLabel('Provenance')).toHaveValue(/Calibrated observation #0/)
  await page.getByRole('button', { name: 'Set estimate' }).click()
  await expect(page.getByText('Point · 0.5')).toBeVisible()
  await expect(page.getByText(/Calibrated observation #0/)).toBeVisible()
})