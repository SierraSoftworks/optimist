import { expect, test } from '@playwright/test'

/**
 * Reviewing a design against its proposals.
 *
 * The metastable example is used because its whole point is that a summary
 * hides what matters: the design settles on two states, and these tests check
 * that the interface says so rather than averaging them into one line.
 */
test.describe('review', () => {
  test('charts the quantities of whatever is under the most pressure', async ({ page }) => {
    await page.goto('/d/metastable/review')

    await expect(page.locator('figure').first()).toBeVisible()
    await expect(page.getByTestId('watch-picker')).toBeVisible()

    // The ranking sits alongside, so a reader can see which limit the chart is
    // about without changing view.
    await expect(page.getByRole('cell', { name: '100%' }).first()).toBeVisible()
  })

  /**
   * A dimensionless quantity living between nought and one is a share of
   * something, and reading it as `0.87` asks for a conversion the model already
   * knows how to do.
   */
  test('reads a proportion as a percentage', async ({ page }) => {
    await page.goto('/d/metastable/review')

    const chart = page.locator('figure', { hasText: 'attempt_success' }).first()
    await expect(chart).toBeVisible()
    // The axis runs the whole range rather than the range the data happened to
    // occupy, because a percentage has a ceiling worth showing.
    await expect(chart.getByText('100%', { exact: true })).toBeVisible()
  })

  test('stopping on a step shows the distribution behind it', async ({ page }) => {
    await page.goto('/d/metastable/review')

    // A quantity that carries a spread. Some channels solve to one certain
    // value, and those are drawn as a point on purpose, so hovering one would
    // prove nothing about the distribution readout.
    const chart = page.locator('figure', { hasText: 'call_latency' }).first()
    await expect(chart).toBeVisible()

    const plot = chart.locator('svg.plot')
    const box = await plot.boundingBox()
    await plot.hover({ position: { x: (box?.width ?? 400) * 0.75, y: (box?.height ?? 60) / 2 } })

    // Beside the point rather than in a footer, so the eye does not travel
    // between the value and the place it came from.
    const readout = chart.getByTestId('step-readout')
    await expect(readout).toBeVisible()
    await expect(readout.getByText(/^t = /)).toBeVisible()
    await expect(readout.locator('svg.sketch')).toBeVisible()
  })

  test('a variant is weighed against the design it would replace', async ({ page }) => {
    await page.goto('/d/metastable/review')

    await page.getByTestId('variant-shed').click()
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

    await page.getByTestId('variant-no-surge').click()
    // Same demand at the moment it is read; different history, different state.
    await expect(page.getByRole('cell', { name: '2.200' })).toHaveCount(0)
  })

  /**
   * Editing an existing variant starts from what it already says.
   *
   * The dialog opened with every field blank once, because the copy it took of
   * the variant threw and was swallowed. It said it was editing something and
   * showed nothing, and saving would have replaced a real proposal with an empty
   * one.
   */
  test('editing a variant starts from what it already says', async ({ page }) => {
    await page.goto('/d/metastable/review')

    // The row's controls appear on hover, as they do in the list this borrows
    // its shape from.
    await page.getByTestId('variant-shed').hover()
    await page.getByLabel('Edit Shed load').click()

    await expect(page.getByTestId('variant-name')).toHaveValue('Shed load')
    await expect(page.getByTestId('variant-id')).toHaveValue('shed')
    await expect(page.getByRole('textbox', { name: 'What this proposes, and why' })).not.toBeEmpty()

    // Its rebinding is what makes it a proposal at all, so it has to be there
    // to be changed.
    await expect(page.getByText('admission_limit')).toBeVisible()
  })

  /**
   * A variant is nothing but a set of replacements for shared quantities, so
   * making one has to be possible without editing a file.
   */
  test('a variant can be created and is then reviewable', async ({ page }) => {
    await page.goto('/d/metastable/review')

    await page.getByTestId('new-variant').click()
    await page.getByTestId('variant-name').fill('Half the depth')
    await page.getByTestId('add-override').click()
    await page
      .locator('.pick-override .el-select-dropdown__item', { hasText: 'call_depth' })
      .first()
      .click()
    await page.getByTestId('save-variant').click()

    // Creating one selects it, so its effect is visible immediately.
    await expect(page).toHaveURL(/\/review\/half-the-depth/)
    await expect(page.getByTestId('variant-half-the-depth')).toBeVisible()

    // And it is part of the design rather than of this tab.
    await page.reload()
    await expect(page.getByTestId('variant-half-the-depth')).toBeVisible()
  })
})
