import { expect, test, type Page } from '@playwright/test'
import { createProject } from '../support/workbench'

async function command(page: Page, value: string) {
  await page.getByRole('button', { name: /Open command bar \((?:Cmd|Ctrl)\+K\)/ }).click()
  const bar = page.getByRole('dialog', { name: 'Command bar' })
  await bar.getByRole('textbox', { name: 'Command', exact: true }).fill(value)
  await expect(bar.getByRole('region', { name: 'Command preview' })).toBeVisible()
  await bar.getByRole('button', { name: 'Apply' }).click()
}

test('applies typed graph commands through the real API', async ({ page }) => {
  await createProject(page, 'Command bar integration')
  await command(page, 'add intervention "Automation"')
  await command(page, 'add factor "Review flow" controllable')
  await command(page, 'connect A changes B 0.4')

  await expect(page.getByText('2 nodes')).toBeVisible()
  await expect(page.getByText('1 relationships')).toBeVisible()
  await expect(page.getByText('changes · mean effect +0.40')).toBeVisible()
  await expect(page.getByRole('button', { name: 'Needs setup 2' })).toBeVisible()
  await expect(page.getByText(/Setup recommended: Success probability, Duration estimate/)).toBeVisible()
})