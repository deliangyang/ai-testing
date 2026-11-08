import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { vi } from 'vitest'

import { SERIAL_DAG } from '../data/fixtures/serial'
import { ASYNC_POLL_DAG } from '../data/fixtures/asyncPoll'
import type { DagVersionDto } from '../data/types'
import { NodePropertyPanel } from './NodePropertyPanel'

describe('NodePropertyPanel', () => {
  it('edits http path and saves pending version', async () => {
    const onDagChange = vi.fn()
    const onSave = vi.fn().mockResolvedValue({
      id: 'serial',
      version: 2,
      status: 'PENDING',
      created_by: 'fixture',
      created_at: '2026-01-01T00:00:00.000Z',
    } satisfies DagVersionDto)

    render(
      <NodePropertyPanel
        dag={SERIAL_DAG}
        selectedNodeId="pay"
        onDagChange={onDagChange}
        onSave={onSave}
      />,
    )

    expect(screen.getByTestId('prop-http-path-suggestions')).toHaveTextContent(
      '${create.output.body}',
    )
    fireEvent.change(screen.getByTestId('prop-http-path'), { target: { value: '/orders/v2' } })
    expect(onDagChange).toHaveBeenCalled()
    fireEvent.click(screen.getByTestId('node-property-save'))

    await waitFor(() => {
      expect(onSave).toHaveBeenCalled()
    })
    expect(screen.getByTestId('dag-pending-save')).toHaveAttribute('data-version', '2')
    expect(screen.getByTestId('dag-pending-save')).toHaveAttribute('data-status', 'PENDING')
  })

  it('shows poll fields for poll nodes', () => {
    render(
      <NodePropertyPanel
        dag={ASYNC_POLL_DAG}
        selectedNodeId="wait_paid"
        onDagChange={vi.fn()}
        onSave={vi.fn()}
      />,
    )
    expect(screen.getByTestId('node-property-panel')).toHaveAttribute('data-node-type', 'poll')
    expect(screen.getByTestId('prop-poll-path')).toBeInTheDocument()
  })

  it('shows assert fields for assert nodes', () => {
    render(
      <NodePropertyPanel
        dag={SERIAL_DAG}
        selectedNodeId="check"
        onDagChange={vi.fn()}
        onSave={vi.fn()}
      />,
    )
    expect(screen.getByTestId('prop-assert-target')).toHaveValue('pay')
  })
})
