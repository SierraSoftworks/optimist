import { expect, test } from '../support/mock-api'

async function apply(page: import('@playwright/test').Page, command: string) {
  const bar = page.getByRole('dialog', { name: 'Command bar' })
  await bar.getByRole('textbox', { name: 'Command', exact: true }).fill(command)
  await expect(bar.getByRole('region', { name: 'Command preview' })).toBeVisible()
  await bar.getByRole('button', { name: 'Apply' }).click()
}

test('creates, connects, and navigates through the command bar', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop keyboard workflow assertion')
  await page.goto('/')
  await page.getByRole('button', { name: 'Create project' }).click()
  await page.getByLabel('Project name').fill('Command workflow')
  await page.getByRole('button', { name: 'Create project' }).last().click()

  const shortcut = await page.evaluate(() => /Mac|iPhone|iPad|iPod/i.test(navigator.platform) ? 'Cmd+K' : 'Ctrl+K')
  const trigger = page.getByRole('button', { name: `Open command bar (${shortcut})` })
  await expect(trigger).toContainText(shortcut)
  await page.screenshot({ path: 'artifacts/command-bar-shortcut.png', fullPage: true })
  await trigger.click()
  await apply(page, 'add intervention "Automation"')
  await expect(page.getByRole('heading', { name: 'Automation' })).toBeVisible()

  await page.keyboard.press('ControlOrMeta+K')
  await apply(page, 'add factor "Review flow" controllable')
  await expect(page.getByRole('heading', { name: 'Review flow' })).toBeVisible()

  await page.keyboard.press('ControlOrMeta+K')
  const contextualBar = page.getByRole('dialog', { name: 'Command bar' })
  await contextualBar.getByRole('textbox', { name: 'Command', exact: true }).fill('connect A ')
  const relationshipKinds = contextualBar.getByRole('listbox', { name: 'Command suggestions' })
  await expect(relationshipKinds.getByRole('option', { name: /Changes/ })).toBeVisible()
  await expect(relationshipKinds.getByRole('option', { name: /Requires/ })).toBeVisible()
  await expect(relationshipKinds.getByRole('option', { name: /Conflicts with/ })).toHaveCount(0)
  await expect(relationshipKinds.getByRole('option', { name: /Synergizes with/ })).toHaveCount(0)
  await contextualBar.getByRole('button', { name: 'Cancel' }).click()

  // A causal relationship needs canonical units on both endpoints, and only a
  // metric can declare its unit while being created.
  await page.keyboard.press('ControlOrMeta+K')
  await apply(page, 'add metric "Deploy rate" deployments/week')
  await expect(page.getByRole('heading', { name: 'Deploy rate' })).toBeVisible()

  await page.keyboard.press('ControlOrMeta+K')
  await apply(page, 'connect A changes C 1 0.35')
  await expect(page.getByText('1 relationships')).toBeVisible()
  await expect(page.getByText('changes · Squiggle response')).toBeVisible()

  await page.keyboard.press('ControlOrMeta+K')
  await apply(page, 'select B')
  await expect(page.getByRole('heading', { name: 'Review flow' })).toBeVisible()

  await page.keyboard.press('ControlOrMeta+K')
  const bar = page.getByRole('dialog', { name: 'Command bar' })
  await bar.getByRole('textbox', { name: 'Command', exact: true }).fill('connect B changes A')
  await expect(bar.getByRole('alert')).toContainText('not valid')
  await expect(bar.getByRole('button', { name: 'Apply' })).toBeDisabled()
  await bar.screenshot({ path: 'artifacts/command-bar-diagnostics.png' })
})