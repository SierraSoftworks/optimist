import { expect, type Page } from '@playwright/test'

export async function createProject(page: Page, name: string) {
  await page.goto('/')
  if (await page.getByRole('heading', { name: 'Create your first project' }).isVisible().catch(() => false)) {
    await page.getByRole('button', { name: 'Create project' }).click()
  } else {
    await page.getByLabel('Project', { exact: true }).selectOption({ label: 'New project...' })
  }
  await page.getByLabel('Project name').fill(name)
  await page.getByRole('form', { name: 'Create project' }).getByRole('button', { name: 'Create project' }).click()
  await expect(page.getByRole('heading', { name: 'Start with a system element' })).toBeVisible()
}

export async function addNode(
  page: Page,
  title: string,
  kind: 'factor' | 'outcome' | 'metric' | 'intervention' = 'factor',
  unit?: string,
) {
  await page.getByRole('button', { name: 'Add node' }).last().click()
  if (kind !== 'factor') await page.getByLabel(kind).check()
  await page.getByLabel('Title').fill(title)
  if (kind === 'metric') await page.getByLabel('Unit').fill(unit ?? 'count')
  await page.getByRole('form', { name: 'Add node' }).getByRole('button', { name: 'Continue' }).click()
  await page.getByRole('button', { name: 'Add ready node' }).click()
  await expect(page.getByRole('heading', { name: title })).toBeVisible()
}

export async function addRelationship(
  page: Page,
  kind: string,
  source: string,
  destination: string,
  options: { effect?: number; mechanism?: string; evidence?: string } = {},
) {
  await page.getByRole('button', { name: 'Relationship', exact: true }).click()
  const form = page.getByRole('form', { name: 'Add relationship' })
  await form.getByRole('combobox').first().selectOption(kind)
  await page.getByLabel('Source').selectOption(source)
  await page.getByLabel('Destination').selectOption(destination)
  if (kind === 'contributes' || kind === 'changes' || kind === 'blocks') {
    await page.getByRole('spinbutton', { name: /effect|degree/i }).fill(String(options.effect ?? 0.5))
  }
  if (options.mechanism !== undefined) await page.getByLabel('Mechanism').fill(options.mechanism)
  if (options.evidence !== undefined) await page.getByLabel('Evidence references').fill(options.evidence)
  await form.getByRole('button', { name: 'Add relationship' }).click()
}

export async function setPointEstimate(page: Page, value: number) {
  await page.getByRole('button', { name: 'Estimate', exact: true }).click()
  await page.getByLabel('Distribution', { exact: true }).selectOption('point')
  await page.getByLabel('Value on [0, 1]').fill(String(value))
  await page.getByRole('button', { name: 'Replace estimate' }).click()
}
