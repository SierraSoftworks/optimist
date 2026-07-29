import { expect, test } from '@playwright/test'

test('property fields preview', async ({ page }) => {
  await page.goto('/d/checkout/design')
  await page.getByTestId('add-component').click()
  await page.getByTestId('component-type-compute').click()
  await page.getByTestId('component-id').fill(`probe${Date.now()}`)
  await page.getByTestId('save-component').click()

  await page.getByTestId('property-service_time').locator('.cm-content').click()
  await page.keyboard.type('0.02 * lognormal(0, 0.2)')
  await expect(page.getByTestId('quantity-preview')).toBeVisible()
  await expect(page.getByTestId('preview-summary')).not.toBeEmpty()
  await expect(page.getByTestId('quantity-preview').locator('svg')).toBeVisible()

  await page.getByRole('heading', { name: 'Properties' }).click()
  await expect(page.getByTestId('quantity-preview')).toHaveCount(0)

  await page.getByTestId('remove-selected').click()
  await page.getByRole('button', { name: 'Yes' }).click()
})

test('scale unit replicas preview', async ({ page }) => {
  await page.goto('/d/metastable/design')
  await page.getByTestId('add-scale-unit').click()
  await page.getByTestId('new-scale-unit-id').fill('cellprobe')
  await page.getByTestId('new-scale-unit-replicas').locator('.cm-content').click()
  await page.keyboard.press('ControlOrMeta+a')
  await page.keyboard.type('7')
  await expect(page.getByTestId('quantity-preview')).toBeVisible()
  await expect(page.getByTestId('preview-value')).toHaveText('7')
})

test('variant override preview', async ({ page }) => {
  await page.goto('/d/metastable/review')
  await page.getByTestId('variant-shed').hover()
  await page.getByLabel('Edit Refuse what cannot be served').click()

  const field = page.getByTestId('override-admission_limit')
  await expect(field).toBeVisible()
  await field.locator('.cm-content').click()
  await expect(page.getByTestId('quantity-preview')).toBeVisible()
  await expect(page.getByTestId('preview-summary')).not.toBeEmpty()
})
