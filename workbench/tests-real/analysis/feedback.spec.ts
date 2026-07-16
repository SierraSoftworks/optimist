import { expect, test } from '@playwright/test'
import { addNode, addRelationship, createProject } from '../support/workbench'

test('finds and highlights a causal feedback loop', async ({ page }) => {
  await createProject(page, 'Feedback analysis')
  await addNode(page, 'Feedback speed')
  await addNode(page, 'Learning rate')
  await addRelationship(page, 'contributes', 'A', 'B', { effect: 0.7, mechanism: 'Feedback improves learning.' })
  await addRelationship(page, 'contributes', 'B', 'A', { effect: 0.3, mechanism: 'Learning reinforces feedback.' })
  await page.getByRole('button', { name: 'Feedback', exact: true }).click()
  const panel = page.getByLabel('Feedback analysis')
  await expect(panel.getByText('A → B → A')).toBeVisible()
  await panel.getByRole('button', { name: /A → B → A/ }).click()
  await expect(page.getByText('Analysis highlights 2 nodes and 2 relationships.')).toBeAttached()
})
