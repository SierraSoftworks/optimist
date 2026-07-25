import { expect, mockApi, test, type FixtureState } from '../support/mock-api'
import { factorNode, outcomeNode } from '../support/fixtures'

function state(): FixtureState {
  return {
    project: { id: 'A', name: 'Shared quantities', revision: 0 },
    revision: 0,
    nodes: [
      factorNode('A', 'Recovery time', { current: 0.4 }),
      factorNode('B', 'Restart time', { current: 0.25 }),
      outcomeNode('C', 'Availability', { current: 0.5 }),
    ],
    edges: [],
  }
}

test('shares one quantity between two estimates instead of duplicating it', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop estimate editing workflow')
  await page.unroute('**/api/v1/**')
  const fixture = state()
  await mockApi(page, fixture)
  await page.goto('/')

  await page.getByRole('button', { name: /Restart time/ }).click()
  await page.getByRole('button', { name: 'Estimate', exact: true }).click()

  const dialog = page.getByRole('form', { name: /state estimate/i })
  await expect(dialog.getByText('Shared quantity')).toBeVisible()

  // Both factors carry the same unit, so each is offered to the other.
  await dialog.getByLabel('Same quantity as').selectOption({ label: 'Recovery time · Current' })
  await dialog.getByRole('button', { name: 'Share this quantity' }).click()

  await expect
    .poll(() => fixture.dependence, { message: 'the coupling must reach the server' })
    .toMatchObject({
      revision: 0,
      residual_groups: [{ correlation: { scale: 'latent', matrix: [[1, 1], [1, 1]] } }],
    })

  // Sharing adopts the partner's definition, which is what makes the marginals
  // identical; a correlation of one between different definitions would only
  // make them comonotonic.
  await expect(page.getByLabel('Squiggle source')).toHaveValue('pointMass(0.4)')
  await expect(dialog.getByText('Recovery time · Current')).toBeVisible()
  await expect(dialog.getByRole('button', { name: 'Stop sharing' })).toBeVisible()

  await dialog.getByRole('button', { name: 'Stop sharing' }).click()
  await expect
    .poll(() => (fixture.dependence as { residual_groups: unknown[] }).residual_groups, {
      message: 'stopping must leave no group behind',
    })
    .toEqual([])
})
