import { expect, test } from '../support/mock-api'

test('guides intervention simulation setup before placing the node', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop wizard assertion')
  await page.goto('/')
  await page.getByRole('button', { name: 'Create project' }).click()
  await page.getByLabel('Project name').fill('Ready interventions')
  await page.getByRole('button', { name: 'Create project' }).last().click()
  await page.getByRole('button', { name: 'Add node' }).last().click()
  await page.getByLabel('intervention').check()
  await page.getByLabel('Title').fill('Automate review')
  await page.getByRole('button', { name: 'Continue' }).click()

  const dialog = page.getByRole('form', { name: 'Planning estimates' })
  await expect(dialog.getByText('Success probability')).toBeVisible()
  await expect(dialog.getByText('Duration', { exact: true })).toBeVisible()
  await dialog.screenshot({ path: 'artifacts/node-readiness-wizard.png' })
  await dialog.getByRole('button', { name: 'Add ready node' }).click()

  await expect(page.getByRole('heading', { name: 'Automate review' })).toBeVisible()
  await expect(page.getByText('Simulation ready')).toBeVisible()
  await expect(page.getByText(/need setup/)).toHaveCount(0)
})