import { expect, test } from '../support/mock-api'

test('confirms Delete-key removal for graph nodes and relationships', async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== 'desktop', 'desktop keyboard workflow')
  await page.goto('/')
  await page.getByRole('button', { name: 'Create project' }).click()
  await page.getByLabel('Project name').fill('Keyboard deletion')
  await page.getByRole('button', { name: 'Create project' }).last().click()

  for (const title of ['Fast feedback', 'Learning rate']) {
    await page.getByRole('button', { name: 'Add node' }).last().click()
    await page.getByLabel('Title').fill(title)
    await page.getByRole('button', { name: 'Add node' }).last().click()
  }
  await page.getByRole('button', { name: 'Relationship', exact: true }).click()
  const relationship = page.getByRole('form', { name: 'Add relationship' })
  await relationship.getByRole('combobox').first().selectOption('part_of')
  await page.getByLabel('Source').selectOption('A')
  await page.getByLabel('Destination').selectOption('B')
  await relationship.getByRole('button', { name: 'Add relationship' }).click()

  await page.getByRole('button', { name: /Fast feedback/ }).click()
  await page.keyboard.press('Delete')
  const blocked = page.getByRole('form', { name: 'Delete node' })
  await expect(blocked.getByText('Delete 1 connected relationship first.')).toBeVisible()
  await expect(blocked.getByRole('button', { name: 'Delete node' })).toBeDisabled()
  await blocked.getByRole('button', { name: 'Cancel' }).click()

  await page.getByRole('button', { name: 'Edit focused relationship A part of B' }).click()
  await page.keyboard.press('Delete')
  const deleteRelationship = page.getByRole('form', { name: 'Delete relationship' })
  await expect(deleteRelationship.getByText('A part of B')).toBeVisible()
  await deleteRelationship.getByRole('button', { name: 'Delete relationship' }).click()
  await expect(page.getByText('0 relationships')).toBeVisible()

  await page.getByRole('button', { name: /Fast feedback/ }).click()
  await page.keyboard.press('Delete')
  await page.getByRole('form', { name: 'Delete node' }).getByRole('button', { name: 'Delete node' }).click()
  await expect(page.getByText('1 nodes')).toBeVisible()
})
