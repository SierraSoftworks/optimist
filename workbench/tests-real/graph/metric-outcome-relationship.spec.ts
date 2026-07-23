import { expect, test } from '@playwright/test'
import { addNode, createProject } from '../support/workbench'

test('creates a Squiggle-authored metric-to-outcome contribution', async ({ page }) => {
  await createProject(page, 'Metric outcome relationship')
  await addNode(page, 'Deployment frequency', 'metric', 'deployments/week')
  await addNode(page, 'Customer impact', 'outcome')

  await page.getByRole('button', { name: 'Relationship', exact: true }).click()
  const form = page.getByRole('form', { name: 'Add relationship' })
  await form.getByRole('combobox').first().selectOption('contributes')
  await form.getByRole('group', { name: 'Source' }).getByText('Deployment frequency', { exact: true }).click()
  await form.getByRole('group', { name: 'Target' }).getByText('Customer impact', { exact: true }).click()
  await form.getByLabel(/Source change/).fill('2')
  await form.getByLabel('Squiggle source').fill('pointMass(0.1)')
  await expect(form.getByText(/Validated/)).toBeVisible()

  const command = page.waitForResponse((response) =>
    response.url().includes('/api/v1/projects/A/commands')
      && response.request().method() === 'POST',
  )
  await form.getByRole('button', { name: 'Add relationship' }).click()

  const response = await command
  expect(response.status()).toBe(201)
  const body = response.request().postDataJSON()
  const estimate = body.command.payload.payload.properties.response.destination_change
  expect(estimate).not.toHaveProperty('distribution')
  expect(estimate.source.definition).toMatchObject({
    source: 'pointMass(0.1)',
    target_unit: {},
  })
  await expect(form).toBeHidden()
  await expect(page.getByRole('alert')).toBeHidden()
})