import { expect, test } from '@playwright/test'

test('creates and reads a typed model through the real Axum API', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('heading', { name: 'Create your first project' })).toBeVisible()

  await page.getByRole('button', { name: 'Create project' }).click()
  await page.getByLabel('Project name').fill('Real API model')
  await page.getByRole('button', { name: 'Create project' }).last().click()
  await expect(page.getByRole('heading', { name: 'Start with a system element' })).toBeVisible()

  await page.getByRole('button', { name: 'Add node' }).last().click()
  await page.getByLabel('Title').fill('Fast feedback')
  await page.getByRole('button', { name: 'Add node' }).last().click()

  await page.getByRole('button', { name: 'Add node' }).first().click()
  await page.getByLabel('Title').fill('Learning rate')
  await page.getByRole('button', { name: 'Add node' }).last().click()

  await page.getByRole('button', { name: 'Relationship' }).click()
  await page.getByLabel('Source').selectOption('A')
  await page.getByLabel('Destination').selectOption('B')
  await page.getByRole('button', { name: 'Add relationship' }).click()

  await expect(page.getByTestId('graph-surface')).toBeVisible()
  await expect(page.getByText('1 relationships')).toBeVisible()
  await page.getByRole('button', { name: /Fast feedback/ }).click()
  await expect(page.getByRole('heading', { name: 'Fast feedback' })).toBeVisible()
  await expect(page.getByText('part of', { exact: true })).toBeVisible()
  await expect(page.getByText('A', { exact: true }).last()).toBeVisible()
  await expect(page.getByText('B', { exact: true }).last()).toBeVisible()
  await expect(page.getByText('r3')).toBeVisible()
})
