import { test, expect, expectCanvasPainted, mockApi, type FixtureState } from '../support/mock-api'

test('creates a typed relationship directly from a right-clicked source node', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop pointer workflow assertion')
  const state: FixtureState = {
    project: { id: 'A', name: 'Direct relationships', revision: 0 },
    revision: 0,
    nodes: [
      {
        id: 'A', revision: 0, name: 'delivery_risk', normalized_name: 'delivery_risk', title: 'Delivery risk',
        description: '', aliases: [], metadata: {},
        payload: { kind: 'factor', properties: { current: null, desired: null, controllable: false, evidence: [] } },
      },
      {
        id: 'B', revision: 0, name: 'automation', normalized_name: 'automation', title: 'Automation',
        description: '', aliases: [], metadata: {},
        payload: { kind: 'intervention', properties: { costs: [], duration: null, probability_of_success: null, acceptance_criteria: [] } },
      },
    ],
    edges: [],
  }
  await page.unroute('**/api/v1/**')
  await mockApi(page, state)
  await page.goto('/')
  await expectCanvasPainted(page)

  const sourcePosition = await page.locator('.graph-canvas canvas[data-id="layer2-node"]').evaluate((element) => {
    const canvas = element as HTMLCanvasElement
    const context = canvas.getContext('2d')!
    const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data
    let count = 0
    let totalX = 0
    let totalY = 0
    for (let y = 0; y < canvas.height; y += 1) {
      for (let x = 0; x < canvas.width; x += 1) {
        const index = (y * canvas.width + x) * 4
        const [red, green, blue, alpha] = pixels.slice(index, index + 4)
        if (alpha! > 200 && red! >= 130 && red! <= 155 && green! >= 170 && green! <= 195 && blue! >= 210 && blue! <= 235) {
          count += 1
          totalX += x
          totalY += y
        }
      }
    }
    if (!count) throw new Error('Could not locate the factor node on the graph canvas')
    const bounds = canvas.getBoundingClientRect()
    return {
      x: bounds.left + (totalX / count) * (bounds.width / canvas.width),
      y: bounds.top + (totalY / count) * (bounds.height / canvas.height),
    }
  })

  await page.mouse.click(sourcePosition.x, sourcePosition.y, { button: 'right' })
  const menu = page.getByRole('menu', { name: 'Add relationship from Delivery risk' })
  await expect(menu).toBeVisible()
  await expect(menu.getByRole('menuitem', { name: 'Blocks' })).toBeVisible()
  await expect(menu.getByRole('menuitem', { name: 'Changes' })).toHaveCount(0)
  await menu.getByRole('menuitem', { name: 'Blocks' }).click()

  const form = page.getByRole('form', { name: 'Add relationship' })
  await expect(form.getByLabel('Source')).toBeDisabled()
  await expect(form.getByLabel('Source')).toHaveValue('A')
  await expect(form.getByLabel('Relationship')).toHaveValue('blocks')
  await form.getByLabel('Destination').selectOption('B')
  await form.getByRole('button', { name: 'Add relationship' }).click()

  await expect(page.getByText('1 relationships')).toBeVisible()
  expect(state.edges).toHaveLength(1)
  expect(state.edges[0]).toMatchObject({ source: 'A', destination: 'B', payload: { kind: 'blocks' } })
})