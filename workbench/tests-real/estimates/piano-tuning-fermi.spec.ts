import { expect, test, type Locator } from '@playwright/test'
import { addNode, createProject } from '../support/workbench'

test('models piano tunings with compact variables, equations, and unit verification', async ({ page }) => {
  await createProject(page, 'Piano tuning estimate')
  await addNode(page, 'Piano tuning demand', 'intervention')
  await page.getByRole('button', { name: /Piano tuning demand/ }).click()
  await page.getByRole('button', { name: 'Edit duration estimate' }).click()
  await page.getByRole('button', { name: 'Fermi equation' }).click()
  await page.getByRole('button', { name: /Fermi decomposition/ }).click()
  const assistant = page.locator('.fermi-assistant')
  await assistant.getByLabel('Fermi goal unit').fill('pianos/day')
  await assistant.getByLabel('Fermi equation').fill('people / people_per_household / households_per_piano / days_per_tuning * pianos_per_tuning')
  await setVariable(assistant, 1, 'people', '1.5M', 'people')
  await setVariable(assistant, 2, 'people_per_household', '3', 'people/household')
  for (let count = 0; count < 3; count += 1) await assistant.getByRole('button', { name: 'Add variable' }).click()
  await setVariable(assistant, 3, 'households_per_piano', '20', 'households/piano')
  await setVariable(assistant, 4, 'days_per_tuning', '180', 'days/tuning')
  await setVariable(assistant, 5, 'pianos_per_tuning', '1', 'pianos/tuning')

  const status = assistant.locator('.fermi-equation-status')
  await expect(status).toContainText('138.889')
  await expect(status).toContainText('piano^2/day')
  await expect(status).toContainText('Unresolved dimension: piano')
  await expect(assistant.getByRole('button', { name: 'Assess equation' })).toBeDisabled()

  await assistant.getByLabel('Variable 4 unit').fill('piano*days/tuning')
  await expect(status).toContainText('Derived unitpiano/day')
  await expect(assistant.getByRole('button', { name: 'Assess equation' })).toBeEnabled()
  await assistant.getByRole('button', { name: 'Assess equation' }).click()

  await expect(assistant.getByText(/20,000 samples · maximum samples reached/)).toBeVisible()
  await expect(assistant.getByText('90% interval', { exact: true })).toBeVisible()
  await expect(assistant.getByText(/standalone assessment only.*expects duration/i)).toBeVisible()
  await expect(assistant.getByRole('button', { name: 'Use suggested distribution' })).toHaveCount(0)
})

async function setVariable(
  assistant: Locator,
  index: number,
  name: string,
  estimate: string,
  unit: string,
) {
  await assistant.getByLabel(`Variable ${index} name`).fill(name)
  await assistant.getByLabel(`Variable ${index} estimate`).fill(estimate)
  await assistant.getByLabel(`Variable ${index} estimate`).blur()
  await assistant.getByLabel(`Variable ${index} unit`).fill(unit)
}
