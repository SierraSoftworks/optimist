import { expect, test } from '@playwright/test'
import { createProject } from '../support/workbench'

test('defines and edits a metric estimate in native units', async ({ page }) => {
  await createProject(page, 'Native metric quantity')
  await page.getByRole('button', { name: 'Add node' }).last().click()
  const dialog = page.locator('.node-dialog')
  await dialog.getByLabel('metric').check()
  await dialog.getByLabel('Title').fill('Lead time')
  await dialog.getByLabel('Unit').fill('days')
  await dialog.getByRole('button', { name: 'Continue' }).click()

  await dialog.getByLabel('Aggregation').fill('p95 weekly')
  await dialog.getByLabel('Support').selectOption('bounded')
  await dialog.getByLabel('Minimum').fill('0')
  await dialog.getByLabel('Maximum').fill('30')
  await dialog.getByLabel('Operational definition').fill('Elapsed calendar days from commit to production for completed changes.')
  await dialog.getByLabel('Reference time').fill('Next 30 days')
  await dialog.getByLabel('Resolution source').fill('Delivery dashboard')
  await dialog.getByLabel('Add a current estimate').check()
  await dialog.getByLabel('Value in days').fill('12')
  await dialog.getByRole('button', { name: 'Add ready node' }).click()

  await expect(page.getByRole('heading', { name: 'Lead time' })).toBeVisible()
  const inspector = page.getByRole('complementary', { name: 'Selection inspector' })
  await expect(inspector.getByText('Native quantity')).toBeVisible()
  await expect(inspector.getByText('0–30')).toBeVisible()
  await expect(inspector.getByText('Point · 12')).toBeVisible()
  await expect(inspector.getByText('Delivery dashboard')).toBeVisible()

  await inspector.getByRole('button', { name: 'Estimate' }).click()
  await expect(page.getByRole('heading', { name: 'Set quantity estimate' })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Fermi equation' })).toHaveCount(0)
  await page.getByLabel('Value in days').fill('10')
  await page.getByRole('button', { name: 'Replace estimate' }).click()

  await expect(inspector.getByText('Point · 10')).toBeVisible()
})
