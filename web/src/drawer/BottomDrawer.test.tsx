import { fireEvent, render, screen } from '@testing-library/react'

import { ASYNC_POLL_FAIL_SCENARIO } from '../data/fixtures/asyncPollFail'
import { extractRunLogEntries } from '../focus/failureFocus'
import { flattenPollTraces } from '../panels/pollTrace'
import { BottomDrawer } from './BottomDrawer'
import { watchVariablesFromExecutions } from './watchVariables'
import './drawer.css'

describe('BottomDrawer', () => {
  const logs = extractRunLogEntries(ASYNC_POLL_FAIL_SCENARIO.events)
  const variables = watchVariablesFromExecutions(ASYNC_POLL_FAIL_SCENARIO.executions)
  const pollRows = flattenPollTraces(ASYNC_POLL_FAIL_SCENARIO.executions)

  it('shows grouped logs with node filter defaulting to failed node', () => {
    render(
      <BottomDrawer
        logs={logs}
        pollRows={pollRows}
        variables={variables}
        initialNodeFilter="wait_paid"
      />,
    )
    expect(screen.getByTestId('run-log-group-wait_paid')).toBeInTheDocument()
    expect(screen.queryByTestId('run-log-group-create')).not.toBeInTheDocument()
    fireEvent.change(screen.getByTestId('run-log-node-filter'), { target: { value: 'all' } })
    expect(screen.getByTestId('run-log-group-create')).toBeInTheDocument()
    expect(screen.getByTestId('run-log-group-pay')).toBeInTheDocument()
  })

  it('switches to variable watch tab', () => {
    render(<BottomDrawer logs={logs} pollRows={pollRows} variables={variables} />)
    fireEvent.click(screen.getByTestId('drawer-tab-variables'))
    expect(screen.getByTestId('variable-watch')).toBeInTheDocument()
    expect(screen.getByTestId('variable-row-create.output.body.id')).toHaveTextContent(
      'o-timeout',
    )
  })
})
