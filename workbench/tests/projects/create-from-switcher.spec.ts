import { test, expect } from '../support/mock-api'

test('creates another project from the project dropdown', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop workflow assertion')
  await page.goto('/')
  await page.getByRole('button', { name: 'Create project' }).click()
  await page.getByLabel('Project name').fill('Existing model')
  await page.getByRole('button', { name: 'Create project' }).last().click()
  const projectSelect = page.getByLabel('Project', { exact: true })
  await expect(projectSelect).toHaveValue('A')

  await projectSelect.selectOption({ label: 'New project...' })
  await expect(page.getByRole('heading', { name: 'Create project' })).toBeVisible()
  await expect(projectSelect).toHaveValue('A')
  await page.getByLabel('Project name').fill('Second model')
  await page.getByRole('button', { name: 'Create project' }).last().click()

  await expect(projectSelect).toHaveValue('B')
  await expect(projectSelect.locator('option')).toHaveText([
    'Existing model',
    'Second model',
    'New project...',
  ])
})
