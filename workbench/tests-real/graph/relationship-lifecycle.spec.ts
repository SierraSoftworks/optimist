import { expect, test } from '@playwright/test'
import { addNode, addRelationship, createProject } from '../support/workbench'

test('creates, edits, deletes, and recreates a relationship', async ({ page }) => {
  await createProject(page, 'Relationship lifecycle')
  await addNode(page, 'Fast feedback')
  await addNode(page, 'Learning rate')
  await addRelationship(page, 'part_of', 'A', 'B')
  await expect(page.getByText('1 relationships')).toBeVisible()

  await page.getByRole('button', { name: /Fast feedback/ }).click()
  await page.getByRole('button', { name: 'Edit part of relationship A to B' }).click()
  await page.getByLabel('Description').fill('Learning is part of feedback.')
  await page.getByLabel('Metadata').fill('{"source":"ADR-2"}')
  await page.getByRole('button', { name: 'Save relationship' }).click()
  await page.getByRole('button', { name: 'Edit part of relationship A to B' }).click()
  await page.getByRole('button', { name: 'Delete', exact: true }).click()
  await page.getByRole('button', { name: 'Confirm delete' }).click()
  await expect(page.getByText('0 relationships')).toBeVisible()
  await addRelationship(page, 'part_of', 'A', 'B')
  await expect(page.getByText('1 relationships')).toBeVisible()
})
