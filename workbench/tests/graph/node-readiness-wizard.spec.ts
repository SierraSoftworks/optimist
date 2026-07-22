import { expect, test } from '../support/mock-api'

test('creates an intervention then guides probabilistic setup through Squiggle', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop wizard assertion')
  await page.goto('/')
  await page.getByRole('button', { name: 'Create project' }).click()
  await page.getByLabel('Project name').fill('Ready interventions')
  await page.getByRole('button', { name: 'Create project' }).last().click()
  await page.getByRole('button', { name: 'Add node' }).last().click()
  await page.getByLabel('intervention').check()
  await page.getByLabel('Title').fill('Automate review')
  await page.getByRole('button', { name: 'Continue' }).click()

  const dialog = page.getByRole('form', { name: 'Action setup' })
  await expect(dialog.getByText('Planning estimates')).toBeVisible()
  await dialog.screenshot({ path: 'artifacts/node-readiness-wizard.png' })
  await dialog.getByRole('button', { name: 'Add node' }).click()

  await expect(page.getByRole('heading', { name: 'Automate review' })).toBeVisible()
  await expect(page.getByText(/Setup recommended/)).toBeVisible()
  await page.getByRole('button', { name: 'Duration estimate', exact: true }).click()
  await expect(page.getByLabel('Squiggle source')).toHaveValue('lognormal(0, 0.5)')
})