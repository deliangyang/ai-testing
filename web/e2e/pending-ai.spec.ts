import { expect, test } from '@playwright/test'

test('ai pending preview diff and activate', async ({ page }) => {
  await page.goto('/?pending=1')
  await expect(page.getByTestId('pending-ai-workbench')).toBeVisible()
  await expect(page.getByTestId('pending-ai-preview-layer')).toBeVisible()
  await expect(page.getByTestId('dag-validation-panel')).toHaveAttribute('data-gate', 'pass')
  await expect(page.getByTestId('pending-diff-added-lookup_stock')).toBeVisible()
  await page.getByTestId('pending-ai-activate').click()
  await expect(page.getByTestId('pending-ai-activated')).toBeVisible()
})
