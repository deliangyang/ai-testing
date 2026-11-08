import { expect, test } from '@playwright/test'

test('live gate validation updates badge and focuses issue node', async ({ page }) => {
  await page.goto('/?edit=1')
  await expect(page.getByTestId('dag-validation-panel')).toHaveAttribute('data-gate', 'pass')
  await page.getByTestId('dag-node-pay').click()
  await page.getByTestId('prop-http-path').fill('/broken/${missing.output.body}')
  await expect(page.getByTestId('dag-validation-panel')).toHaveAttribute('data-gate', 'fail')
  await page.getByTestId('gate-issue-pay').click()
  await expect(page.getByTestId('dag-edit-canvas')).toHaveAttribute('data-focus-node', 'pay')
})
