import { expect, test } from '@playwright/test'

test('gate failure offers AI repair and restores passing validation', async ({ page }) => {
  await page.goto('/?edit=1')
  await expect(page.getByTestId('dag-validation-panel')).toHaveAttribute('data-gate', 'pass')
  await page.getByTestId('dag-node-pay').click()
  await page.getByTestId('prop-http-path').fill('/broken/${missing.output.body}')
  await expect(page.getByTestId('dag-validation-panel')).toHaveAttribute('data-gate', 'fail')
  await page.getByTestId('gate-ai-repair').click()
  await expect(page.getByTestId('dag-validation-panel')).toHaveAttribute('data-gate', 'pass', {
    timeout: 15_000,
  })
  await expect(page.getByTestId('gate-ai-repair-hint')).toContainText('待审')
})
