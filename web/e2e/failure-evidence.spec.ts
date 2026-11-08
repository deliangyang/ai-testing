import { expect, test } from '@playwright/test'

/**
 * develop.md §8.1 Q2 / P1-29: from a failed (red) poll node to full evidence in ≤2 clicks.
 */
test('poll timeout failure shows full evidence within two clicks', async ({ page }) => {
  let userClicks = 0
  page.on('click', () => {
    userClicks += 1
  })

  await page.goto('/')

  const failedNode = page.getByTestId('dag-run-node-wait_paid')
  await expect(failedNode).toHaveAttribute('data-status', 'FAILED')

  const detail = page.getByTestId('execution-detail')
  const timelineVisible = await detail
    .getByTestId('poll-timeline-wait_paid-1')
    .isVisible()
    .catch(() => false)

  if (!timelineVisible) {
    await failedNode.click()
    await expect(detail).toContainText('wait_paid')
  }

  await expect(detail.getByTestId('poll-timeline-wait_paid-1')).toBeVisible()
  await expect(detail.getByTestId('poll-timeline-wait_paid-4')).toBeVisible()

  expect(userClicks).toBeLessThanOrEqual(2)
})
