import { expect, test } from '@playwright/test'

test('edit http node and save pending dag version', async ({ page }) => {
  await page.goto('/?edit=1')
  await expect(page.getByTestId('dag-edit-workbench')).toBeVisible()
  await page.getByTestId('dag-node-create').click()
  await expect(page.getByTestId('node-property-panel')).toHaveAttribute('data-node-type', 'http')
  await page.getByTestId('prop-http-path').fill('/orders/edited')
  await page.getByTestId('node-property-save').click()
  await expect(page.getByTestId('dag-pending-save')).toHaveAttribute('data-version', '2')
  await expect(page.getByTestId('dag-pending-save')).toHaveAttribute('data-status', 'PENDING')
})
