import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { FixtureDataSource } from '../data/fixture'
import { SERIAL_AI_PENDING_DETAIL } from './fixtures'
import { PendingAiReviewPanel } from './PendingAiReviewPanel'

describe('PendingAiReviewPanel', () => {
  it('半透明预览、校验徽标与差异列表', async () => {
    const dataSource = new FixtureDataSource('serial')
    render(
      <PendingAiReviewPanel dataSource={dataSource} initialPending={SERIAL_AI_PENDING_DETAIL} />,
    )
    await waitFor(() => {
      expect(screen.getByTestId('pending-ai-preview-layer')).toBeInTheDocument()
    })
    expect(screen.getByTestId('dag-validation-panel')).toHaveAttribute('data-gate', 'pass')
    expect(screen.getByTestId('pending-diff-added-lookup_stock')).toBeInTheDocument()
    expect(screen.getByTestId('pending-ai-activate')).not.toBeDisabled()
  })

  it('激活成功后显示确认文案', async () => {
    const dataSource = new FixtureDataSource('serial')
    render(
      <PendingAiReviewPanel dataSource={dataSource} initialPending={SERIAL_AI_PENDING_DETAIL} />,
    )
    await waitFor(() => {
      expect(screen.getByTestId('pending-ai-activate')).not.toBeDisabled()
    })
    fireEvent.click(screen.getByTestId('pending-ai-activate'))
    await waitFor(() => {
      expect(screen.getByTestId('pending-ai-activated')).toBeInTheDocument()
    })
  })
})
