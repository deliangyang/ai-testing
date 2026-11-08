import { expect, test } from '@playwright/test'

test.describe('top bar run controls', () => {
  test('run then stop returns to idle', async ({ page }) => {
    await page.goto('/')
    await expect(page.getByTestId('run-control-run')).toBeEnabled()
    await page.getByTestId('run-control-run').click()
    await expect(page.getByTestId('run-phase')).toHaveText('running')
    await expect(page.getByTestId('run-control-stop')).toBeEnabled()
    await page.getByTestId('run-control-stop').click()
    await expect(page.getByTestId('run-phase')).toHaveText('idle')
  })

  test('environment selector updates connectivity dot', async ({ page }) => {
    await page.goto('/')
    await expect(page.getByTestId('env-connectivity-dot')).toHaveClass(/ok/)
    await page.getByTestId('env-select-input').selectOption('stg')
    await expect(page.getByTestId('env-connectivity-dot')).toHaveClass(/bad/)
    await expect(page.getByTestId('status-env')).toContainText('stg')
  })

  test('rerun failed is enabled on async-poll failure workspace', async ({ page }) => {
    await page.goto('/')
    await expect(page.getByTestId('run-control-rerun-failed')).toBeEnabled()
  })
})
