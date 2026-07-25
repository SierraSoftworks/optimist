import { expect, mockApi, test, type FixtureState } from '../support/mock-api'
import { causalEdge, factorNode, interventionNode, outcomeNode } from '../support/fixtures'

test('orders causal hierarchy and exposes focused relationship metadata', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop hierarchy assertion')
  const state: FixtureState = {
    project: { id: 'A', name: 'Hierarchy review', revision: 0 },
    revision: 0,
    nodes: [
      interventionNode('A', 'Pair review', { duration: 2, probability: 0.8 }),
      factorNode('B', 'Review flow', { controllable: true, current: 0.45 }),
      outcomeNode('C', 'Reliable delivery'),
    ],
    edges: [
      causalEdge('A', 'intervention', 'B', 'factor', {
        kind: 'changes',
        response: 0.35,
        mechanism: 'Automates checks',
      }),
      causalEdge('B', 'factor', 'C', 'outcome', {
        response: 0.6,
        mechanism: 'Shortens review',
      }),
    ],
  }
  await page.unroute('**/api/v1/**')
  await mockApi(page, state)
  await page.goto('/')
  await page.getByRole('button', { name: /Review flow/ }).click()

  await expect(page.getByText('1 need setup')).toBeVisible()
  await page.getByRole('button', { name: 'Needs setup 1' }).click()
  await expect(page.getByText('1 nodes')).toBeVisible()
  await expect(page.getByRole('button', { name: /Reliable delivery/ })).toBeVisible()
  await expect(page.getByRole('button', { name: /Pair review/ })).toHaveCount(0)
  await page.getByRole('button', { name: 'Needs setup 1' }).click()
  await page.getByRole('button', { name: /Review flow/ }).click()
  await page.getByRole('button', { name: 'Cluster by kind' }).click()
  const clusters = page.getByLabel('Node kind clusters')
  await expect(clusters).toContainText('Actions 1')
  await expect(clusters).toContainText('Factors 1')
  await expect(clusters).toContainText('Objectives 1')
  await page.screenshot({ path: 'artifacts/graph-kind-clusters.png', fullPage: true })
  await page.getByRole('button', { name: 'Hierarchy layout' }).click()
  const focused = page.getByRole('region', { name: 'Focused relationships' })
  await expect(focused.getByText('changes · Squiggle response')).toBeVisible()
  await expect(focused.getByText('contributes · Squiggle response')).toBeVisible()

  const centers = await page.locator('.graph-canvas canvas[data-id="layer2-node"]').evaluate((element) => {
    const canvas = element as HTMLCanvasElement
    const context = canvas.getContext('2d')!
    const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data
    const colors = {
      intervention: [232, 152, 115],
      factor: [142, 182, 224],
      outcome: [245, 184, 63],
    } as const
    const sums = Object.fromEntries(Object.keys(colors).map((kind) => [kind, { y: 0, count: 0 }])) as Record<string, { y: number; count: number }>
    for (let y = 0; y < canvas.height; y += 1) {
      for (let x = 0; x < canvas.width; x += 1) {
        const offset = (y * canvas.width + x) * 4
        for (const [kind, color] of Object.entries(colors)) {
          if (Math.abs(pixels[offset]! - color[0]) < 5 && Math.abs(pixels[offset + 1]! - color[1]) < 5 && Math.abs(pixels[offset + 2]! - color[2]) < 5 && pixels[offset + 3]! > 200) {
            sums[kind]!.y += y
            sums[kind]!.count += 1
          }
        }
      }
    }
    return Object.fromEntries(Object.entries(sums).map(([kind, value]) => [kind, value.y / value.count]))
  })
  expect(centers.intervention).toBeLessThan(centers.factor!)
  expect(centers.factor).toBeLessThan(centers.outcome!)

  await focused.getByRole('button', { name: 'Edit focused relationship B contributes C' }).click()
  await expect(page.getByRole('heading', { name: 'Edit relationship' })).toBeVisible()
  await page.getByRole('button', { name: 'Close' }).click()
  await page.screenshot({ path: 'artifacts/graph-hierarchy-readiness.png', fullPage: true })
})