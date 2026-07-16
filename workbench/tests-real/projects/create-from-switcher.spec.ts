import { expect, test } from '@playwright/test'
import { createProject } from '../support/workbench'

test('creates another project from the active project switcher', async ({ page }) => {
  await createProject(page, 'First real project')
  const projectSelect = page.getByLabel('Project', { exact: true })
  const firstProjectId = await projectSelect.inputValue()
  await projectSelect.selectOption({ label: 'New project...' })
  await expect(projectSelect).toHaveValue(firstProjectId)
  await page.getByLabel('Project name').fill('Second real project')
  await page.getByRole('form', { name: 'Create project' }).getByRole('button', { name: 'Create project' }).click()
  await expect(projectSelect).not.toHaveValue(firstProjectId)
  await expect(projectSelect.locator('option:checked')).toHaveText('Second real project')
  await expect(page.getByRole('heading', { name: 'Start with a system element' })).toBeVisible()
})
