import { expect, type Page } from '@playwright/test'

export async function createProject(page: Page, name: string) {
  await page.goto('/')
  if (await page.getByRole('heading', { name: 'Create your first project' }).isVisible().catch(() => false)) {
    await page.getByRole('button', { name: 'Create project' }).click()
  } else {
    await page.getByLabel('Project', { exact: true }).selectOption({ label: 'New project...' })
  }
  await page.getByLabel('Project name').fill(name)
  const form = page.getByRole('form', { name: 'Create project' })
  await form.getByRole('button', { name: 'Create project' }).click()
  await expect(form).toBeHidden()
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
  const form = page.getByRole('form', { name: 'Add node' })
  await form.getByRole('button', { name: 'Add node' }).click()
  await expect(form).toBeHidden()
  await expect(page.getByRole('heading', { name: title })).toBeVisible()
  if (kind === 'factor' || kind === 'outcome') {
    await page.getByRole('button', { name: 'Native state' }).click()
    const quantity = page.getByRole('form', { name: 'Configure native state' })
    await quantity.getByLabel('Unit').fill('state')
    await quantity.getByLabel('Support').selectOption('bounded')
    await quantity.getByLabel('Lower bound').fill('0')
    await quantity.getByLabel('Upper bound').fill('1')
    await quantity.getByRole('button', { name: 'Use native state' }).click()
    await expect(quantity).toBeHidden()
    await page.getByRole('button', { name: 'Estimate', exact: true }).click()
    await saveSquiggleEstimate(page, 'beta(2, 2)', 'Set estimate')
  }
  if (kind === 'intervention') {
    await page.getByRole('button', { name: 'Duration estimate', exact: true }).click()
    await saveSquiggleEstimate(page, 'lognormal(1.38629436112, 0.35)', 'Set estimate')
    await page.getByRole('button', { name: 'Success probability', exact: true }).click()
    await saveSquiggleEstimate(page, 'beta(4, 2)', 'Set estimate')
  }
}

export async function saveSquiggleEstimate(page: Page, source: string, command: 'Set estimate' | 'Replace estimate') {
  await page.getByLabel('Squiggle source').fill(source)
  await expect(page.getByText(/effective samples/)).toBeVisible()
  await page.getByRole('button', { name: command }).click()
  await expect(page.getByLabel('Squiggle source')).toBeHidden()
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
  await form.locator(`input[name="relationship-source"][value="${source}"]`).locator('..').click()
  await form.locator(`input[name="relationship-destination"][value="${destination}"]`).locator('..').click()
  if (kind === 'contributes' || kind === 'changes') {
    await form.getByLabel('Squiggle source').fill(`pointMass(${options.effect ?? 0.5})`)
    await expect(form.getByText(/Validated/)).toBeVisible()
  } else if (kind === 'blocks') {
    await form.getByLabel(/Blocking degree/).fill(String(options.effect ?? 0.5))
  }
  await form.getByRole('button', { name: 'Add relationship' }).click()
  await expect(form).toBeHidden()
}

export async function setPointEstimate(page: Page, value: number) {
  await page.getByRole('button', { name: 'Estimate', exact: true }).click()
  await saveSquiggleEstimate(page, `pointMass(${value})`, 'Replace estimate')
}
