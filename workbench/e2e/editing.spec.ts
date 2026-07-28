import { expect, test } from '@playwright/test'

/**
 * Editing, and knowing whether an edit landed.
 *
 * The first version of this workbench bound every field straight to server
 * state. Fields could not be typed into whenever anything else in the design was
 * invalid, and nothing said why. These are the tests that would have caught it.
 */
test.describe('editing', () => {
  test('a field accepts typing and saves when it is left', async ({ page }) => {
    await page.goto('/d/checkout/design')

    const editor = page.getByTestId('quantity-peak_rate').locator('.cm-content')
    await expect(editor).toHaveText('900')

    await editor.click()
    await page.keyboard.press('End')
    await page.keyboard.type(' * 2')
    // Typed text is on screen before anything is sent, which is the property
    // that binding to the model directly destroys.
    await expect(editor).toHaveText('900 * 2')

    // Leaving the field is what commits it.
    await page.getByRole('heading', { name: 'Shared quantities' }).click()
    await page.reload()
    await expect(page.getByTestId('quantity-peak_rate').locator('.cm-content')).toHaveText('900 * 2')

    // Put it back, so the rest of the run sees the design it expects.
    const restored = page.getByTestId('quantity-peak_rate').locator('.cm-content')
    await restored.click()
    await page.keyboard.press('ControlOrMeta+a')
    await page.keyboard.type('900')
    await page.getByRole('heading', { name: 'Shared quantities' }).click()
    await expect(page.getByTestId('quantity-peak_rate').locator('.cm-content')).toHaveText('900')
  })

  /**
   * What a quantity is for is written before the design that gives it a
   * purpose, so the first description is usually the wrong one. Fixing it used
   * to mean deleting the quantity and every reference to it.
   */
  test('the description of a quantity can be rewritten', async ({ page }) => {
    await page.goto('/d/checkout/design')

    await page.getByTestId('describe-peak_rate').click()
    const description = page.getByTestId('quantity-description')
    await expect(description).toHaveValue('Requests per second at the daily peak.')

    await description.fill('Requests per second during the Friday evening peak.')
    await page.getByTestId('save-description').click()

    await page.reload()
    await page.getByTestId('describe-peak_rate').click()
    await expect(page.getByTestId('quantity-description')).toHaveValue(
      'Requests per second during the Friday evening peak.',
    )

    // Put it back, so the rest of the run sees the design it expects.
    await page.getByTestId('quantity-description').fill('Requests per second at the daily peak.')
    await page.getByTestId('save-description').click()
  })

  /**
   * A component with an empty property cannot be solved, and until it is filled
   * in every analysis of the design fails. Saying nothing about that is what
   * made the tool look broken rather than half-finished.
   */
  test('says why a half-finished design will not solve', async ({ page }) => {
    const id = `incomplete-${Date.now()}`

    await page.goto('/')
    await page.getByTestId('new-design').click()
    await page.getByTestId('design-name').fill('Incomplete')
    await page.getByTestId('design-id').fill(id)
    await page.getByTestId('create-design').click()
    await expect(page).toHaveURL(new RegExp(`/d/${id}/design`))

    await page.getByRole('button', { name: 'Add a component' }).click()
    await page.getByTestId('component-type-compute').click()
    await page.getByTestId('component-id').fill('api')
    await page.getByTestId('save-component').click()

    // The inspector names the properties still needed rather than leaving the
    // reader to discover them one failed solve at a time.
    await expect(page.getByTestId('unfilled-warning')).toBeVisible()
    await expect(page.getByTestId('unfilled-warning')).toContainText('service_time')

    // And the toolbar says the design as a whole will not solve.
    await expect(page.getByTestId('solve-problem')).toBeVisible()
  })

  /**
   * Layout is part of what a design says, so a placement has to reach the
   * server and come back.
   *
   * Cytoscape draws to a canvas, so a node cannot be addressed as an element and
   * a drag cannot be aimed at one reliably. What is checked here is the half
   * that can be: a position the design already carries survives a reload and is
   * handed to the diagram rather than being laid out over. Whether a drag
   * produces one is covered by the schema test in the Rust suite.
   */
  test('keeps a placement across a reload', async ({ page }) => {
    await page.request.post('/api/v1/designs/checkout/mutations', {
      data: {
        mutations: [
          {
            kind: 'set_component',
            component: {
              id: 'browsers',
              name: 'Browsers',
              type: 'client',
              properties: { request_rate: 'peak_rate', payload: '512' },
              position: { x: 321, y: 123 },
            },
          },
        ],
      },
    })

    await page.goto('/d/checkout/design')
    await expect(page.locator('.graph')).toBeVisible()

    const response = await page.request.get('/api/v1/designs/checkout')
    const snapshot = (await response.json()) as {
      model: { components: { id: string; position?: { x: number; y: number } }[] }
    }
    const placed = snapshot.model.components.find((component) => component.id === 'browsers')
    expect(placed?.position).toEqual({ x: 321, y: 123 })
  })
})
