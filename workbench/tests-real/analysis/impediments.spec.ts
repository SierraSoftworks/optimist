import { expect, test } from '@playwright/test'
import { addNode, addRelationship, createProject } from '../support/workbench'

test('projects and highlights impediment candidates', async ({ page }) => {
  await createProject(page, 'Impediment analysis')
  await addNode(page, 'Review throughput')
  await addNode(page, 'Delivery reliability', 'outcome')
  await addRelationship(page, 'contributes', 'A', 'B', { effect: 0.5, evidence: 'ADR-1' })
  await page.getByRole('button', { name: 'Impediments', exact: true }).click()
  const panel = page.getByLabel('Impediments analysis')
  await expect(panel.getByText('Review throughput', { exact: true })).toBeVisible()
  await panel.getByRole('button', { name: /Evidence/ }).click()
  await panel.getByRole('button', { name: /Review throughput A/ }).click()
  await expect(page.getByText('Analysis highlights 2 nodes and 1 relationships.')).toBeAttached()
  await expect(panel.getByText(/Neither is a causal confidence score/)).toBeVisible()
})
