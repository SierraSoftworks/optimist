import { expect, test } from '@playwright/test'
import { addNode, createProject } from '../support/workbench'

test('sets, replaces, and removes intervention estimates', async ({ page }) => {
  await createProject(page, 'Intervention estimates')
  await addNode(page, 'Automate review', 'intervention')
  await page.getByRole('button', { name: /Automate review/ }).click()
  await page.getByRole('button', { name: 'Edit duration estimate' }).click()
  await page.getByLabel('Distribution', { exact: true }).selectOption('log_normal')
  await page.getByRole('spinbutton', { name: 'Log location', exact: true }).fill('2')
  await page.getByRole('spinbutton', { name: 'Log scale', exact: true }).fill('0.3')
  await page.getByRole('button', { name: 'Replace estimate' }).click()
  await expect(page.getByText('LogNormal · μ 2, σ 0.3')).toBeVisible()

  await page.getByRole('button', { name: 'Add cost dimension' }).click()
  await page.getByRole('textbox', { name: 'Dimension', exact: true }).fill('engineer_days')
  await page.getByRole('spinbutton', { name: 'Value', exact: true }).fill('12')
  await page.getByRole('button', { name: 'Set estimate' }).click()
  await page.getByRole('button', { name: 'Edit engineer_days cost estimate' }).click()
  await page.getByRole('spinbutton', { name: 'Value', exact: true }).fill('10')
  await page.getByRole('button', { name: 'Replace estimate' }).click()
  await expect(page.getByText('Point · 10')).toBeVisible()
  await page.getByRole('button', { name: 'Edit engineer_days cost estimate' }).click()
  await page.getByRole('button', { name: 'Remove', exact: true }).click()
  await page.getByRole('button', { name: 'Confirm remove' }).click()
  await expect(page.getByText('No cost dimensions configured.')).toBeVisible()
})
