import { render, screen, within } from '@testing-library/react'

import { DagRunCanvas } from '../canvas'
import { ASYNC_POLL_FAIL_SCENARIO } from '../data/fixtures/asyncPollFail'
import { RunLogList } from '../drawer/RunLogList'
import '../drawer/drawer.css'
import { ExecutionDetailPanel, PollTraceTable } from '../panels'
import { useFailureFocus } from './useFailureFocus'

function FailureFocusHarness() {
  const focus = useFailureFocus(ASYNC_POLL_FAIL_SCENARIO)
  return (
    <div data-testid="failure-focus-harness" data-focus-node={focus.failedNodeId ?? ''}>
      <DagRunCanvas dag={focus.dag} events={focus.events} focusNodeId={focus.failedNodeId} />
      <ExecutionDetailPanel nodeId={focus.failedNodeId ?? ''} execution={focus.execution} />
      <RunLogList entries={focus.filteredLogs} focusNodeId={focus.failedNodeId} />
      <PollTraceTable rows={focus.pollRows} />
    </div>
  )
}

describe('failure focus workspace', () => {
  it('centers failed run state on wait_paid with detail and filtered logs', () => {
    render(<FailureFocusHarness />)
    expect(screen.getByTestId('failure-focus-harness')).toHaveAttribute(
      'data-focus-node',
      'wait_paid',
    )
    expect(screen.getByTestId('dag-run-canvas')).toHaveAttribute('data-focus-node', 'wait_paid')
    expect(screen.getByTestId('dag-run-node-wait_paid')).toHaveAttribute('data-status', 'FAILED')
    expect(screen.getByTestId('execution-detail')).toHaveTextContent('wait_paid')
    expect(screen.getByTestId('execution-failure')).toHaveTextContent('timeout')
    const logs = screen.getByTestId('run-log-list')
    expect(logs).toHaveAttribute('data-focus-node', 'wait_paid')
    expect(within(logs).getAllByRole('listitem')).toHaveLength(1)
    expect(screen.getByTestId('poll-trace-row-wait_paid-1')).toBeInTheDocument()
  })
})
