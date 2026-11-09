import { expect, test } from '@playwright/test'

test('copilot tab mock chat binds canvas actions', async ({ page }) => {
  await page.goto('/?edit=1')
  await expect(page.getByTestId('dag-edit-workbench')).toBeVisible()
  await page.getByTestId('right-tab-copilot').click()
  await expect(page.getByTestId('copilot-panel')).toBeVisible()
  await expect(page.getByTestId('copilot-context-dag')).toHaveTextContent('serial')
  await page.getByTestId('dag-node-pay').click()
  await expect(page.getByTestId('copilot-context-node')).toHaveTextContent('pay')
  await page.getByTestId('copilot-send').click()
  await expect(page.getByTestId('copilot-action-insert_canvas-0')).toBeVisible()
  await page.getByTestId('copilot-action-insert_canvas-0').click()
  await expect(page.getByTestId('dag-node-health')).toBeVisible()
  await page.getByTestId('copilot-action-adopt_assertion-2').click()
  await page.getByTestId('right-tab-props').click()
  await page.getByTestId('dag-node-check').click()
  await expect(page.getByTestId('prop-assert-path')).toHaveValue('$.body.paid')
})
