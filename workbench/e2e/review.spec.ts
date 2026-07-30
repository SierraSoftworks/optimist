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
  test('charts the quantities of whatever is under the most pressure', async ({ page }) => {
    await page.goto('/d/metastable/review')

    // A solved chart rather than the outline standing in for one, so that a
    // skeleton left on screen forever would fail this rather than satisfy it.
    await expect(page.locator('figure svg.plot').first()).toBeVisible()
    await expect(page.getByTestId('watch-picker')).toBeVisible()

    // What the design is closest to exhausting sits in the header, so a reader
    // can see which limit the chart is about without changing view.
    await expect(page.getByTestId('limit-cards')).toBeVisible()
  })

  /**
   * A first solve has nothing to show, so it shows the shape of what is coming.
   *
   * The alternative was an empty panel and a badge in a corner, which read as a
   * page that had finished loading and found nothing.
   */
  test('outlines the charts while the first answer is still being solved', async ({ page }) => {
    await page.route('**/analysis*', async (route) => {
      await new Promise((resume) => setTimeout(resume, 1500))
      await route.continue()
    })
    await page.goto('/d/metastable/review')

    await expect(page.getByTestId('chart-skeleton').first()).toBeVisible()
    await expect(page.getByTestId('solve-progress')).toBeVisible()

    await expect(page.locator('figure svg.plot').first()).toBeVisible({ timeout: 30_000 })
    await expect(page.getByTestId('chart-skeleton')).toHaveCount(0)
  })

  /**
   * A dimensionless quantity living between nought and one is a share of
   * something, and reading it as `0.87` asks for a conversion the model already
   * knows how to do.
   */
  test('reads a proportion as a percentage', async ({ page }) => {
    await page.goto('/d/metastable/review')

    const chart = page.locator('figure', { hasText: 'Success rate' }).first()
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
    const chart = page.locator('figure', { hasText: 'Response time' }).first()
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

  /**
   * What a proposal did to each limit, beside the design's own name.
   *
   * It used to be a table in a panel down the side, which meant the answer to
   * "did this help" was one tab away from the charts that showed how.
   */
  test('a variant is weighed against the design it would replace', async ({ page }) => {
    await page.goto('/d/metastable/review')
    await expect(page.getByTestId('limit-cards')).toBeVisible()

    await page.getByTestId('variant-shed').click()

    // Shedding is the lever that ends this collapse, so the constraint it
    // relieves must say so. A card that reported no movement would mean the
    // variant never reached the solver.
    await expect(page.getByTestId('limit-cards').getByText('%').first()).toBeVisible()
    await expect(page.getByTestId('limit-cards')).toContainText(/no longer binds|−/, {
      timeout: 30_000,
    })
  })

  /**
   * A variant's charts carry the design they would replace.
   *
   * A proposal is judged by the distance between two lines, and reading that off
   * two charts on two screens is not something anybody does accurately. The
   * baseline is therefore drawn on the same axes, dashed so it cannot be
   * mistaken for the result.
   */
  test('charts a variant against the baseline it would replace', async ({ page }) => {
    await page.goto('/d/metastable/review')
    await expect(page.locator('figure svg.plot').first()).toBeVisible()
    await expect(page.locator('polyline.reference')).toHaveCount(0)

    await page.getByTestId('variant-shed').click()

    const chart = page.locator('figure', { has: page.locator('svg.plot') }).first()
    await expect(chart.locator('polyline.reference')).toBeVisible({ timeout: 30_000 })
    await expect(chart.getByTestId('baseline-legend')).toContainText('as designed')

    // And the gap between them is named rather than left to the eye.
    const plot = chart.locator('svg.plot')
    const box = await plot.boundingBox()
    await plot.hover({ position: { x: (box?.width ?? 400) * 0.75, y: (box?.height ?? 60) / 2 } })
    await expect(chart.getByTestId('baseline-shift')).toContainText('vs as designed')
  })

  /**
   * Results that are no longer about the question on screen must not read as if
   * they are.
   *
   * Choosing a variant leaves the previous answer mounted while the new one is
   * solved. A reader who takes those numbers for the variant they just chose
   * concludes something about a design nobody solved, which is worse than any
   * amount of waiting — so what is retained is covered until it catches up.
   */
  test('covers the previous variant while the chosen one is solved', async ({ page }) => {
    await page.goto('/d/metastable/review')
    await expect(page.locator('figure svg.plot').first()).toBeVisible()

    await page.route('**/analysis*', async (route) => {
      await new Promise((resume) => setTimeout(resume, 2000))
      await route.continue()
    })
    // Read from the sidebar rather than written in here, so the assertion is
    // about the variant that was chosen rather than about its current wording.
    const chosen = page.getByTestId('variant-shed')
    const name = (await chosen.innerText()).trim()
    await chosen.click()

    const veil = page.getByTestId('solving-veil').first()
    await expect(veil).toBeVisible()
    await expect(veil).toContainText(name)

    await page.unroute('**/analysis*')
    await expect(page.getByTestId('solving-veil')).toHaveCount(0, { timeout: 30_000 })
  })

  /**
   * Choosing what to watch sits beside choosing what to watch it against.
   *
   * It was a multi-select above the charts, which closed after every pick and
   * made assembling a set of four an exercise in patience.
   */
  test('quantities are pinned from the sidebar', async ({ page }) => {
    await page.goto('/d/metastable/review')
    await expect(page.locator('figure svg.plot').first()).toBeVisible()

    const picker = page.getByTestId('watch-picker')
    const charted = await page.locator('figure', { has: page.locator('svg.plot') }).count()

    await picker.getByTestId('signal-search').fill('utilisation')
    const first = picker.locator('[data-test^="pin-"]').first()
    const value = ((await first.getAttribute('data-test')) ?? '').replace('pin-', '')
    await first.click()

    await expect(picker.getByTestId(`unpin-${value}`)).toBeVisible()
    await expect(page.locator('figure', { has: page.locator('svg.plot') })).toHaveCount(charted + 1)

    await picker.getByTestId('clear-signals').click()
    await expect(picker.locator('[data-test^="unpin-"]')).toHaveCount(0)
  })

  /**
   * The order of the watched list is the order of the charts.
   *
   * Which makes it the reader's statement of what this design is about, and a
   * list that can only be appended to forces them to clear it and start again to
   * put the number they care about at the top.
   */
  test('watched quantities are reordered by dragging, and survive a trip to the design', async ({
    page,
  }) => {
    await page.goto('/d/metastable/review')
    await expect(page.locator('figure svg.plot').first()).toBeVisible()

    const watched = () =>
      page
        .getByTestId('watch-picker')
        .locator('.pinned li')
        .evaluateAll((rows) => rows.map((row) => row.getAttribute('data-test') ?? ''))

    const before = await watched()
    expect(before.length).toBeGreaterThan(1)

    // Below the halfway line of the last row, which is what puts it after.
    await page
      .getByTestId(before[0])
      .dragTo(page.getByTestId(before[before.length - 1]), { targetPosition: { x: 60, y: 30 } })

    const after = [...before.slice(1), before[0]]
    await expect.poll(watched).toEqual(after)

    await chooseRadio(page, 'Design')
    await expect(page).toHaveURL(/\/d\/metastable\/design/)
    await chooseRadio(page, 'Simulation')

    await expect.poll(watched).toEqual(after)
  })

  test('the counterfactual and the design differ', async ({ page }) => {
    await page.goto('/d/metastable/review')
    const cards = page.getByTestId('limit-cards')
    await expect(cards).toBeVisible()
    const collapsing = await cards.innerText()

    await page.getByTestId('variant-no-surge').click()
    // Same demand at the moment it is read; different history, different state.
    await expect(cards).not.toHaveText(collapsing, { timeout: 30_000 })
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
    await page.getByLabel('Edit Refuse what cannot be served').click()

    await expect(page.getByTestId('variant-name')).toHaveValue('Refuse what cannot be served')
    await expect(page.getByTestId('variant-id')).toHaveValue('shed')
    await expect(page.getByRole('textbox', { name: 'What this proposes, and why' })).not.toBeEmpty()

    // Its rebinding is what makes it a proposal at all, so it has to be there
    // to be changed. Named in the dialog specifically: hovering the row to
    // reach its controls also opens the row's own summary, which says the same
    // thing without offering to change it.
    await expect(page.getByTestId('override-admission_limit')).toBeVisible()
  })

  /**
   * What a variant proposes, without opening it.
   *
   * The rail has room for a name and nothing else, so a reader deciding which
   * of five proposals to look at had to open each one to find out what it did.
   * The same panel carries the solve, so a row turning over says what it is
   * working on rather than only that it is busy.
   */
  test('a variant says what it proposes when it is hovered', async ({ page }) => {
    await page.goto('/d/metastable/review')
    await page.getByTestId('variant-shed').hover()

    const about = page.getByTestId('about-shed')
    await expect(about).toBeVisible()
    await expect(about).toContainText('Refuse what cannot be served')
    await expect(about).toContainText('admission_limit')
  })

  /**
   * A variant is nothing but a set of replacements for shared quantities, so
   * making one has to be possible without editing a file.
   */
  test('a variant can be created and is then reviewable', async ({ page }) => {
    await page.goto('/d/metastable/review')

    await page.getByTestId('new-variant').click()
    await page.getByTestId('variant-name').fill('Half the attempts')
    await page.getByTestId('add-override').click()
    await page
      .locator('.pick-override .el-select-dropdown__item', { hasText: 'max_attempts' })
      .first()
      .click()
    await page.getByTestId('save-variant').click()

    // Creating one selects it, so its effect is visible immediately.
    await expect(page).toHaveURL(/\/review\/half-the-attempts/)
    await expect(page.getByTestId('variant-half-the-attempts')).toBeVisible()

    // And it is part of the design rather than of this tab.
    await page.reload()
    await expect(page.getByTestId('variant-half-the-attempts')).toBeVisible()
  })
})
