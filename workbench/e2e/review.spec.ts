import { expect, test } from '@playwright/test'

import { chooseRadio } from './support/controls'

/**
 * Reviewing a design against its proposals.
 *
 * The metastable example is used because its whole point is that a summary
 * hides what matters: the design settles on two states, and these tests check
 * that the interface says so rather than averaging them into one line.
 */
test.describe('review', () => {
  test('charts the quantities that feed a constraint', async ({ page }) => {
    await page.goto('/d/metastable/review')

    // Something is charted without being asked for, chosen from what is
    // actually constrained.
    await expect(page.locator('figure').first()).toBeVisible()
    await expect(page.getByTestId('watch-picker')).toBeVisible()

    // The ranking sits alongside, so a reader can see which limit the chart is
    // about without changing view.
    await expect(page.getByRole('cell', { name: '100%' }).first()).toBeVisible()
  })

  test('stopping on a step shows the distribution behind it', async ({ page }) => {
    await page.goto('/d/metastable/review')

    // A quantity that carries a spread. Some channels solve to one certain
    // value, and those are drawn as a point on purpose, so hovering one would
    // prove nothing about the distribution readout.
    const chart = page.locator('figure', { hasText: 'call_latency' }).first()
    await expect(chart).toBeVisible()
    await expect(chart.getByText('Hover to read a step')).toBeVisible()

    const plot = chart.locator('svg.plot')
    // `hover` rather than a raw pointer move: the chart may be below the fold,
    // and a bounding box read from a scrolled-out element points at nothing.
    const box = await plot.boundingBox()
    await plot.hover({ position: { x: (box?.width ?? 400) * 0.75, y: (box?.height ?? 60) / 2 } })

    // A median alone cannot say whether a value is one outcome or two, so the
    // readout carries the spread and the shape rather than a single number.
    await expect(chart.getByText(/^t = /)).toBeVisible()
    await expect(chart.locator('svg.sketch')).toBeVisible()
  })

  test('a variant is weighed against the design it would replace', async ({ page }) => {
    await page.goto('/d/metastable/review')

    await chooseRadio(page, 'Shed load')
    await expect(page.getByRole('tab', { name: 'Against baseline' })).toBeVisible()
    await page.getByRole('tab', { name: 'Against baseline' }).click()

    // Shedding is the lever that ends this collapse, so something must be
    // reported as relieved. A comparison that found nothing would mean the
    // variant never reached the solver.
    await expect(page.getByText('relieved').first()).toBeVisible()
  })

  test('the counterfactual and the design differ', async ({ page }) => {
    await page.goto('/d/metastable/review')
    await expect(page.getByRole('cell', { name: '2.200' }).first()).toBeVisible()

    await chooseRadio(page, 'Never have the surge')
    // Same demand at the moment it is read; different history, different state.
    await expect(page.getByRole('cell', { name: '2.200' })).toHaveCount(0)
  })
})
