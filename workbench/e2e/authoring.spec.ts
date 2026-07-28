import { expect, test } from '@playwright/test'

import { chooseOption } from './support/controls'

/**
 * Building a design from nothing.
 *
 * This is the path a new user takes and the one most likely to be broken by a
 * change elsewhere, because it touches creation, the catalogue, the graph, the
 * inspector and the change feed in one sequence.
 */
test('a design can be created and built up', async ({ page }) => {
  const id = `scratch-${Date.now()}`

  await page.goto('/')
  await page.getByTestId('new-design').click()
  await page.getByTestId('design-name').fill('Scratch')
  await page.getByTestId('design-id').fill(id)
  await page.getByTestId('create-design').click()

  await expect(page).toHaveURL(new RegExp(`/d/${id}/design`))
  await expect(page.getByText('Nothing here yet')).toBeVisible()

  // A component comes from the catalogue rather than being invented, so the
  // type gallery is the only way in.
  await page.getByRole('button', { name: 'Add a component' }).click()
  await page.getByTestId('component-type-client').click()
  await page.getByTestId('component-id').fill('users')
  await page.getByTestId('save-component').click()

  // The inspector opens on what was just added, which is where its properties
  // are filled in.
  await expect(page.getByTestId('component-name')).toHaveValue('users')

  await page.getByTestId('add-component').click()
  await page.getByTestId('component-type-compute').click()
  await page.getByTestId('component-id').fill('api')
  await page.getByTestId('save-component').click()

  await page.getByTestId('add-relationship').click()
  await page.getByTestId('connect-from').click()
  await chooseOption(page, 'pick-from', 'users')
  await page.getByTestId('connect-to-select').click()
  await chooseOption(page, 'pick-to', 'api')
  await page.getByTestId('save-relationship').click()

  // Queue depth belongs to the wire rather than to any behaviour attached to
  // it, so the inspector edits it on the relationship itself.
  await page.getByTestId('relationship-capacity').locator('.cm-content').click()
  await page.keyboard.type('16')
  await page.getByRole('heading', { name: 'Queue depth' }).click()

  // Reloading proves the edits reached the server rather than living in the tab.
  await page.reload()
  await expect(page.locator('canvas').first()).toBeVisible()

  const snapshot = (await (await page.request.get(`/api/v1/designs/${id}`)).json()) as {
    model: { relationships: { from: string; to: string; capacity?: string }[] }
  }
  expect(snapshot.model.relationships[0]?.capacity).toBe('16')

  await page.goto('/d/' + id + '/review')
  await expect(page.getByRole('navigation', { name: 'Variants' })).toBeVisible()
})

test('a shared quantity can be added', async ({ page }) => {
  const id = `quantities-${Date.now()}`

  await page.goto('/')
  await page.getByTestId('new-design').click()
  await page.getByTestId('design-name').fill('Quantities')
  await page.getByTestId('design-id').fill(id)
  await page.getByTestId('create-design').click()
  await expect(page).toHaveURL(new RegExp(`/d/${id}/design`))

  await page.getByTestId('add-quantity').click()
  await page.getByTestId('new-quantity-name').fill('peak_rate')
  await page.getByTestId('new-quantity-expression').locator('.cm-content').fill('900')
  await page.getByTestId('save-quantity').click()

  // The expression is a real editor, so the assertion is on its content rather
  // than on an input value. Editing one is covered in the editing suite, which
  // is where the save-on-blur behaviour belongs.
  await expect(page.getByTestId('quantity-peak_rate').locator('.cm-content')).toHaveText('900')
})

/**
 * Grouping components into a replicated boundary, and nesting one boundary in
 * another.
 *
 * The nesting is the part worth an end-to-end test: it is the only place the
 * editor has to refuse an arrangement the server would reject, and the count a
 * reader acts on is the product along the chain rather than either number on
 * its own.
 */
test('components can be grouped into nested scale units', async ({ page }) => {
  const id = `cells-${Date.now()}`

  await page.goto('/')
  await page.getByTestId('new-design').click()
  await page.getByTestId('design-name').fill('Cells')
  await page.getByTestId('design-id').fill(id)
  await page.getByTestId('create-design').click()
  await expect(page).toHaveURL(new RegExp(`/d/${id}/design`))

  await page.getByRole('button', { name: 'Add a component' }).click()
  await page.getByTestId('component-type-compute').click()
  await page.getByTestId('component-id').fill('api')
  await page.getByTestId('save-component').click()

  // Reached from the toolbar with a component selected, which is when somebody
  // thinks of grouping it and when the panel is not on screen.
  await page.getByTestId('add-scale-unit-toolbar').click()
  await page.getByTestId('new-scale-unit-id').fill('cell')
  await page.getByTestId('new-scale-unit-name').fill('Serving cell')
  await page.getByTestId('new-scale-unit-replicas').locator('.cm-content').fill('12')
  await page.getByTestId('new-scale-unit-members').click()
  await chooseOption(page, 'pick-new-members', 'api')
  await page.getByTestId('save-scale-unit').click()

  await expect(page.getByTestId('scale-unit-tally-cell')).toContainText('Deployed 12 times')

  await page.getByTestId('add-scale-unit').click()
  await page.getByTestId('new-scale-unit-id').fill('region')
  await page.getByTestId('new-scale-unit-replicas').locator('.cm-content').fill('3')
  await page.getByTestId('save-scale-unit').click()

  await page.getByTestId('scale-unit-parent-cell').click()
  await chooseOption(page, 'pick-parent-cell', 'region')

  // Twelve cells in each of three regions is thirty-six copies, which is the
  // number the design is actually sized against.
  await expect(page.getByTestId('scale-unit-tally-cell')).toContainText('Deployed 12 × 3 times')

  // A component belongs to one unit, so the other cannot offer to take it.
  await page.getByTestId('scale-unit-members-region').click()
  await expect(
    page.locator('.pick-members-region .el-select-dropdown__item').filter({ hasText: 'api' }),
  ).toHaveClass(/is-disabled/)
})
