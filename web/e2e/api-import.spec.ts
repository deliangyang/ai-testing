import { expect, test } from '@playwright/test'

test('import preview highlights low confidence and commits accepted rows', async ({ page }) => {
  await page.goto('/?import=1')
  await expect(page.getByTestId('api-import-workbench')).toBeVisible()
  const lowCard = page.getByTestId('import-card-get.orders.id')
  await expect(lowCard).toHaveAttribute('data-low-confidence', 'true')
  await page.getByTestId('import-path-get.orders.id').fill('/orders/{orderId}')
  await page.getByTestId('import-accept-get.orders.id').check()
  await page.getByTestId('import-commit').click()
  await expect(page.getByTestId('import-commit-result')).toContainText('已入库')
})
