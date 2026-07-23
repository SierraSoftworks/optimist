import { expect, mockApi, test, type FixtureState } from '../support/mock-api'

const rejection = 'Failed to deserialize the JSON body into the target type: command.payload.payload.properties.response.destination_change.distribution: unknown field `distribution`, expected one of `id`, `revision`, `quantity`, `source`, `provenance`, `uncertainty` at line 1 column 320'

test('presents relationship contract failures with recovery guidance', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop relationship toolbar workflow')
  const state: FixtureState = {
    project: { id: 'A', name: 'Relationship errors', revision: 0 },
    revision: 0,
    nodes: [
      {
        id: 'A', revision: 0, name: 'deployment_frequency', normalized_name: 'deployment_frequency', title: 'Deployment frequency',
        description: '', aliases: [], metadata: {},
        payload: {
          kind: 'metric',
          properties: {
            quantity: { unit: 'deployments/week', dimension: { deployment: 1, week: -1 }, aggregation: null },
            current: null,
          },
        },
      },
      {
        id: 'B', revision: 0, name: 'customer_impact', normalized_name: 'customer_impact', title: 'Customer impact',
        description: '', aliases: [], metadata: {},
        native_state: {
          quantity: { unit: 'state', dimension: {}, aggregation: null, support: { type: 'bounded', lower: 0, upper: 1 } },
          current: null,
          forecast: null,
        },
        payload: { kind: 'outcome', properties: { direction: 'maximize', evidence: [] } },
      },
    ],
    edges: [],
  }
  await page.unroute('**/api/v1/**')
  await mockApi(page, state)
  await page.route('**/api/v1/projects/A/commands', (route) =>
    route.fulfill({ status: 422, contentType: 'text/plain', body: rejection }),
  )
  await page.goto('/')

  await page.getByRole('button', { name: 'Relationship', exact: true }).click()
  const form = page.getByRole('form', { name: 'Add relationship' })
  await form.getByRole('combobox').first().selectOption('contributes')
  await form.getByRole('group', { name: 'Source' }).getByText('Deployment frequency', { exact: true }).click()
  await form.getByRole('group', { name: 'Target' }).getByText('Customer impact', { exact: true }).click()
  await expect(form.getByText(/Validated/)).toBeVisible()
  await form.getByRole('button', { name: 'Add relationship' }).click()

  const alert = page.getByRole('alert')
  await expect(alert).toContainText('The server rejected the “distribution” field in the submitted relationship estimate.')
  await expect(alert).toContainText('Refresh the page before retrying')
  await expect(alert).toContainText('destination_change.distribution')
  await expect(form).toBeVisible()
})