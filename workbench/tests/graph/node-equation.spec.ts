import { expect, mockApi, test, type FixtureState } from '../support/mock-api'
import { causalEdge, metricNode, outcomeNode } from '../support/fixtures'

function state(): FixtureState {
  return {
    project: { id: 'A', name: 'Node equations', revision: 0 },
    revision: 0,
    nodes: [
      metricNode('A', 'Outage frequency', 'outages', { outage: 1 }, 4),
      metricNode('B', 'Impact duration', 'minutes per outage', { minute: 1, outage: -1 }, 30),
      outcomeNode('C', 'Customer impact', { direction: 'minimize', current: 0.5 }),
    ],
    edges: [
      causalEdge('A', 'metric', 'C', 'outcome'),
      causalEdge('B', 'metric', 'C', 'outcome'),
    ],
  }
}

test('writes a node equation over the parents the graph provides', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop inspector workflow')
  await page.unroute('**/api/v1/**')
  const fixture = state()
  await mockApi(page, fixture)
  await page.goto('/')

  await page.getByRole('button', { name: /Customer impact/ }).click()
  await expect(page.getByText('Composed from the proportional responses')).toBeVisible()
  await page.getByRole('button', { name: 'Add equation' }).click()

  const dialog = page.getByRole('form', { name: 'Node equation' })
  // The bindings come from the graph, so the author never has to guess a name.
  await expect(dialog.getByText('outage_frequency', { exact: true })).toBeVisible()
  await expect(dialog.getByText('impact_duration', { exact: true })).toBeVisible()
  await expect(dialog.getByText('baseline', { exact: true })).toBeVisible()

  await dialog.getByLabel('Calculation').fill('outage_frequency * impact_duration')
  await dialog.getByRole('button', { name: 'Add equation' }).click()

  await expect
    .poll(() => (fixture.nodes[2]!.native_state as { relation?: { source: string } }).relation, {
      message: 'the equation must reach the server',
    })
    .toMatchObject({ source: 'outage_frequency * impact_duration' })

  // The inspector reflects that composition is now the equation's job.
  await expect(page.getByText('outage_frequency * impact_duration')).toBeVisible()
  await page.getByRole('button', { name: 'Edit equation' }).click()
  await dialog.getByRole('button', { name: 'Remove equation' }).click()
  await expect
    .poll(() => (fixture.nodes[2]!.native_state as { relation?: unknown }).relation, {
      message: 'removing must clear the equation',
    })
    .toBeNull()
})
