import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { FixtureDataSource } from '../data/fixture'
import { MARKDOWN_IMPORT_PREVIEW } from './fixtures'
import { ApiImportPanel } from './ApiImportPanel'
import { LOW_CONFIDENCE_THRESHOLD } from './tokens'

describe('ApiImportPanel', () => {
  it('高亮低置信度预览卡片并支持修正路径', () => {
    const dataSource = new FixtureDataSource('serial')
    render(<ApiImportPanel dataSource={dataSource} initialPreview={MARKDOWN_IMPORT_PREVIEW} />)

    const low = MARKDOWN_IMPORT_PREVIEW.items.find(
      (row) => row.confidence < LOW_CONFIDENCE_THRESHOLD,
    )
    if (low === undefined) {
      throw new Error('fixture must include a low-confidence row')
    }
    const card = screen.getByTestId(`import-card-${low.spec.id}`)
    expect(card).toHaveAttribute('data-low-confidence', 'true')

    const pathInput = screen.getByTestId(`import-path-${low.spec.id}`)
    fireEvent.change(pathInput, { target: { value: '/orders/{orderId}' } })
    expect(pathInput).toHaveValue('/orders/{orderId}')
  })

  it('切换合并策略后重新预览', async () => {
    const dataSource = new FixtureDataSource('serial')
    render(<ApiImportPanel dataSource={dataSource} initialPreview={MARKDOWN_IMPORT_PREVIEW} />)
    fireEvent.click(screen.getByLabelText('跳过已有'))
    fireEvent.click(screen.getByTestId('import-preview'))
    await vi.waitFor(() => {
      expect(screen.getByTestId('import-summary')).toHaveTextContent('预览')
    })
  })
})
