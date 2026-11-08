import { expect, test } from '@playwright/test'

test.describe('partial rerun from node', () => {
  test('context menu triggers partial rerun intent on failed node', async ({ page }) => {
    await page.goto('/')
    await page.getByTestId('dag-run-node-wait_paid').click({ button: 'right' })
    await expect(page.getByTestId('run-node-context-menu')).toBeVisible()
    await page.getByTestId('run-node-ctx-rerun').click()
    await expect(page.getByTestId('dag-run-canvas')).toHaveAttribute('data-partial-rerun-node', 'wait_paid')
    await expect(page.getByTestId('dag-run-canvas')).toHaveAttribute(
      'data-partial-rerun-from-run',
      'run-async-poll-fail',
    )
  })

  test('replay bar reruns from selected node on report workspace', async ({ page }) => {
    await page.goto('/')
    await page.getByTestId('nav-report-run-async-poll-fail').dblclick()
    await expect(page.getByTestId('run-replay-actions')).toBeVisible()
    await page.getByTestId('dag-run-node-wait_paid').click()
    await page.getByTestId('replay-rerun-from-node').click()
    await expect(page.getByTestId('dag-run-canvas')).toHaveAttribute('data-partial-rerun-node', 'wait_paid')
    await expect(page.getByTestId('dag-run-canvas')).toHaveAttribute(
      'data-partial-rerun-from-run',
      'run-async-poll-fail',
    )
  })
})
