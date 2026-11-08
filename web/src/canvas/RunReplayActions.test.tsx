import { fireEvent, render, screen } from '@testing-library/react'
import { vi } from 'vitest'

import { RunReplayActions } from './RunReplayActions'

describe('RunReplayActions', () => {
  it('calls onRerunFromNode for the selected node', () => {
    const onRerun = vi.fn()
    render(
      <RunReplayActions
        selectedNodeId="wait_paid"
        sourceRunId="run-async-poll-fail"
        onRerunFromNode={onRerun}
      />,
    )
    fireEvent.click(screen.getByTestId('replay-rerun-from-node'))
    expect(onRerun).toHaveBeenCalledWith('wait_paid')
  })

  it('disables the button when nothing is selected', () => {
    render(<RunReplayActions selectedNodeId={null} onRerunFromNode={vi.fn()} />)
    expect(screen.getByTestId('replay-rerun-from-node')).toBeDisabled()
  })
})
