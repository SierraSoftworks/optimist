import { expect, test } from '@playwright/test'

import { chooseRadio } from './support/controls'

/**
 * The URL is the record of what somebody is looking at, so a link has to restore
 * it. These check the addressable state round-trips rather than that the router
 * is configured, which is the part a colleague notices when it is wrong.
 */
test.describe('routing', () => {
  test('lands on the design picker and opens a design', async ({ page }) => {
    await page.goto('/')
    await expect(page.getByRole('heading', { name: 'Designs' })).toBeVisible()

    await page.getByTestId('open-metastable').click()
    await expect(page).toHaveURL(/\/d\/metastable\/design/)
    await expect(page.getByTestId('mode-switch')).toBeVisible()
  })

  test('a deep link restores the design and its mode', async ({ page }) => {
    await page.goto('/d/checkout/review')
    await expect(page.getByRole('navigation', { name: /Variants/ })).toBeVisible()
    await expect(page.getByRole('radio', { name: 'Simulation' })).toBeChecked()
  })

  test('switching mode changes the address', async ({ page }) => {
    await page.goto('/d/checkout/design')
    await chooseRadio(page, 'Simulation')
    await expect(page).toHaveURL(/\/d\/checkout\/review/)

    await chooseRadio(page, 'Design')
    await expect(page).toHaveURL(/\/d\/checkout\/design/)
  })

  test('selecting a variant is addressable', async ({ page }) => {
    await page.goto('/d/metastable/review')
    await page.getByTestId('variant-shed').click()
    await expect(page).toHaveURL(/\/d\/metastable\/review\/shed/)

    // The same link, opened cold, comes back to the same variant.
    await page.goto('/d/metastable/review/shed')
    await expect(page.getByRole('heading', { name: 'Refuse what cannot be served' })).toBeVisible()
  })

  test('an unknown address falls back to the picker', async ({ page }) => {
    await page.goto('/nonsense/path')
    await expect(page.getByRole('heading', { name: 'Designs' })).toBeVisible()
  })
})
