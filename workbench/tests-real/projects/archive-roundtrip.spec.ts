import { expect, test } from '@playwright/test'
import { addNode, createProject, setPointEstimate } from '../support/workbench'

test('exports and restores a portable project archive', async ({ page }) => {
  await createProject(page, 'Archive roundtrip')
  await addNode(page, 'Baseline factor')
  await setPointEstimate(page, 0.65)
  const downloadPromise = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Export project' }).click()
  const download = await downloadPromise
  const archivePath = await download.path()
  expect(archivePath).not.toBeNull()

  await addNode(page, 'Temporary factor')
  await expect(page.getByText('2 nodes')).toBeVisible()
  const projectId = await page.getByLabel('Project', { exact: true }).inputValue()
  await page.getByRole('button', { name: 'Import project' }).click()
  await page.locator('input[type="file"]').setInputFiles(archivePath!)
  await page.getByLabel(`Type ${projectId} to confirm`).fill(projectId)
  await page.getByRole('button', { name: 'Replace project' }).click()
  await expect(page.getByText('1 nodes')).toBeVisible()
  await expect(page.getByRole('button', { name: /Temporary factor/ })).toHaveCount(0)
  await page.getByRole('button', { name: /Baseline factor/ }).click()
  await expect(page.getByText('Empirical · 2,048 samples')).toBeVisible()
  await expect(page.getByText(/pointMass\(0.65\) · PointMass/)).toBeVisible()
})
