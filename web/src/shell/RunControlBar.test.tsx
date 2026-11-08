import { fireEvent, render, screen } from '@testing-library/react'

import { RunControlBar } from './RunControlBar'
import { initialRunControlState } from './runControl'

describe('RunControlBar', () => {
  it('enables run when idle and stop when running', () => {
    const onRun = vi.fn()
    const onStop = vi.fn()
    const { rerender } = render(
      <RunControlBar
        state={initialRunControlState}
        onRun={onRun}
        onStop={onStop}
        onRerunFailed={() => {}}
        onModeChange={() => {}}
      />,
    )
    expect(screen.getByTestId('run-control-run')).toBeEnabled()
    expect(screen.getByTestId('run-control-stop')).toBeDisabled()
    fireEvent.click(screen.getByTestId('run-control-run'))
    expect(onRun).toHaveBeenCalled()
    rerender(
      <RunControlBar
        state={{ ...initialRunControlState, phase: 'running' }}
        onRun={onRun}
        onStop={onStop}
        onRerunFailed={() => {}}
        onModeChange={() => {}}
      />,
    )
    expect(screen.getByTestId('run-control-stop')).toBeEnabled()
    expect(screen.getByTestId('run-control-run')).toBeDisabled()
  })

  it('enables rerun failed only when allowed', () => {
    render(
      <RunControlBar
        state={{ ...initialRunControlState, canRerunFailed: true }}
        onRun={() => {}}
        onStop={() => {}}
        onRerunFailed={() => {}}
        onModeChange={() => {}}
      />,
    )
    expect(screen.getByTestId('run-control-rerun-failed')).toBeEnabled()
  })
})
