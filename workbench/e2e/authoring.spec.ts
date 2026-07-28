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
