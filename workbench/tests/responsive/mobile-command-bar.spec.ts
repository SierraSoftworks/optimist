import { expect, test } from '../support/mock-api'

test('keeps command preview and apply controls usable on mobile', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'mobile', 'mobile-only command workflow')
  await page.goto('/')
  await page.getByRole('button', { name: 'Create project' }).click()
  await page.getByLabel('Project name').fill('Mobile commands')
  await page.getByRole('button', { name: 'Create project' }).last().click()
  await page.getByRole('button', { name: 'Open command bar' }).click()

  const bar = page.getByRole('dialog', { name: 'Command bar' })
  await bar.getByRole('textbox', { name: 'Command', exact: true }).fill('add outcome "Customer trust" minimize')
  await expect(bar.getByText('Current Beta(2, 2)')).toBeVisible()
  const box = await bar.boundingBox()
  expect(box).not.toBeNull()
  expect(box!.x).toBeGreaterThanOrEqual(0)
  expect(box!.x + box!.width).toBeLessThanOrEqual(page.viewportSize()!.width)
  await bar.screenshot({ path: 'artifacts/mobile-command-bar.png' })
  await bar.getByRole('button', { name: 'Apply' }).click()
  await expect(page.getByRole('heading', { name: 'Customer trust' })).toBeVisible()
})