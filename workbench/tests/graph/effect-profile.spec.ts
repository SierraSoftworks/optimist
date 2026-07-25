import { expect, mockApi, test, type FixtureState } from '../support/mock-api'

const CHANGE_RATE_UNIT = { change: 1, month: -1 }

function state(): FixtureState {
  return {
    project: { id: 'A', name: 'Time-boxed interventions', revision: 0 },
    revision: 0,
    nodes: [
      {
        id: 'A', revision: 0, name: 'code_yellow', normalized_name: 'code_yellow', title: 'Code yellow',
        description: '', aliases: [], metadata: {},
        payload: {
          kind: 'intervention',
          properties: { costs: [], duration: null, probability_of_success: null, acceptance_criteria: [] },
        },
      },
      {
        id: 'B', revision: 0, name: 'change_frequency', normalized_name: 'change_frequency', title: 'Change frequency',
        description: '', aliases: [], metadata: {},
        native_state: {
          quantity: {
            unit: 'changes/month',
            dimension: CHANGE_RATE_UNIT,
            aggregation: null,
            support: { type: 'non_negative' },
          },
          current: null,
          forecast: null,
        },
        payload: { kind: 'factor', properties: { controllable: false, evidence: [] } },
      },
    ],
    edges: [
      {
        source: 'A', source_kind: 'intervention', destination: 'B', destination_kind: 'factor',
        revision: 0, description: '', metadata: {},
        payload: {
          kind: 'changes',
          properties: {
            response: {
              id: 'A',
              revision: 0,
              source: {
                type: 'squiggle',
                definition: {
                  source: 'pointMass(0.1)',
                  seed: 42,
                  sample_count: 256,
                  target_unit: {},
                },
              },
              provenance: [],
            },
            lag: null,
            mechanism: '',
            evidence: [],
          },
        },
      },
    ],
  }
}

test('time-boxes an intervention effect and records its rebound', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop relationship editing workflow')
  await page.unroute('**/api/v1/**')
  const fixture = state()
  await mockApi(page, fixture)
  await page.goto('/')

  await page.getByRole('button', { name: /Code yellow/ }).click()
  await page.getByRole('button', { name: 'Edit focused relationship A changes B' }).click()

  const dialog = page.getByRole('dialog', { name: 'Edit relationship' })
  await expect(dialog.getByText('Effect profile')).toBeVisible()

  await dialog.getByLabel('Time-box this intervention').check()
  await dialog.getByLabel('Hold (periods)').fill('2')
  await dialog.getByLabel('Ending this intervention has its own effect').check()
  await dialog.getByLabel('Rebound multiplier').fill('1.25')
  await dialog.getByLabel('Rebound holds for (periods)').fill('1')

  // The preview must show the pulse and its rebound before anything is saved.
  await expect(dialog.getByRole('img', { name: /period 1: 100%/ })).toBeVisible()
  await expect(dialog.getByRole('img', { name: /period 3: 0%/ })).toBeVisible()
  await expect(dialog.getByText('Rebound', { exact: true })).toBeVisible()

  await dialog.getByRole('button', { name: 'Save profile' }).click()

  await expect
    .poll(() => fixture.edges[0]!.revision, { message: 'the profile must reach the server' })
    .toBe(1)
  const properties = (fixture.edges[0]!.payload as {
    properties: {
      transience: {
        profile: { hold: { source: { definition: { source: string } } } }
        rebound: { source: { definition: { source: string } } }
      } | null
    }
  }).properties
  expect(properties.transience?.profile.hold.source.definition.source).toBe('pointMass(2)')
  expect(properties.transience?.rebound.source.definition.source).toBe('pointMass(1.25)')

  // Reopening must recover the authored shape rather than resetting the form.
  await dialog.getByRole('button', { name: 'Close' }).click()
  await page.getByRole('button', { name: 'Edit focused relationship A changes B' }).click()
  await expect(dialog.getByLabel('Hold (periods)')).toHaveValue('2')
  await expect(dialog.getByLabel('Rebound multiplier')).toHaveValue('1.25')
})

test('records the reviewable claim behind a relationship', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop relationship editing workflow')
  await page.unroute('**/api/v1/**')
  const fixture = state()
  await mockApi(page, fixture)
  await page.goto('/')

  await page.getByRole('button', { name: /Code yellow/ }).click()
  await page.getByRole('button', { name: 'Edit focused relationship A changes B' }).click()

  const dialog = page.getByRole('dialog', { name: 'Edit relationship' })
  await dialog.getByLabel('Mechanism').fill('A freeze suppresses the defect inflow.')
  await dialog.getByLabel('Evidence').fill('2026-Q2 retrospective\nIncident review 4831')
  await dialog.getByRole('button', { name: 'Save claim' }).click()

  await expect
    .poll(() => fixture.edges[0]!.revision, { message: 'the claim must reach the server' })
    .toBe(1)
  const properties = (fixture.edges[0]!.payload as {
    properties: { mechanism: string; evidence: string[] }
  }).properties
  expect(properties.mechanism).toBe('A freeze suppresses the defect inflow.')
  expect(properties.evidence).toEqual(['2026-Q2 retrospective', 'Incident review 4831'])
})
