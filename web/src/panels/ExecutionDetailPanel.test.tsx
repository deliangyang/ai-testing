import { render, screen, within } from '@testing-library/react'

import { FAILED_ASSERT_EXECUTION } from '../data/fixtures/failedAssert'
import { POLL_TIMEOUT_EXECUTION } from '../data/fixtures/pollTimeout'
import { SERIAL_EXECUTIONS } from '../data/fixtures/serial'
import { ExecutionDetailPanel } from './ExecutionDetailPanel'

describe('ExecutionDetailPanel', () => {
  it('shows empty state without execution', () => {
    render(<ExecutionDetailPanel nodeId="pay" execution={null} />)
    expect(screen.getByText(/选择节点/)).toBeInTheDocument()
  })

  it('renders http request/response and timing for pay', () => {
    const pay = SERIAL_EXECUTIONS.find((e) => e.node_id === 'pay')
    expect(pay).toBeDefined()
    if (!pay) {
      return
    }
    render(<ExecutionDetailPanel nodeId="pay" execution={pay} />)
    expect(screen.getByTestId('execution-request')).toHaveTextContent('POST')
    expect(screen.getByTestId('execution-request')).toHaveTextContent('/orders/o-fixture/pay')
    expect(screen.getByTestId('execution-response')).toHaveTextContent('"paid": true')
    expect(screen.getByTestId('execution-timing')).toHaveTextContent('700 ms')
    expect(screen.getByTestId('execution-timing')).toHaveTextContent('42 ms')
  })

  it('renders passing assert output', () => {
    const check = SERIAL_EXECUTIONS.find((e) => e.node_id === 'check')
    expect(check).toBeDefined()
    if (!check) {
      return
    }
    render(<ExecutionDetailPanel nodeId="check" execution={check} />)
    const row = screen.getByTestId('assertion-$.status')
    expect(within(row).getByText('通过')).toBeInTheDocument()
    expect(screen.queryByTestId('assertion-diff')).not.toBeInTheDocument()
  })

  it('renders poll trace timeline for timeout execution', () => {
    render(<ExecutionDetailPanel nodeId="wait_paid" execution={POLL_TIMEOUT_EXECUTION} />)
    expect(screen.getByTestId('poll-trace-timeline')).toBeInTheDocument()
    expect(screen.getByTestId('poll-timeline-wait_paid-1')).toBeInTheDocument()
    expect(screen.getByTestId('poll-timeline-wait_paid-4')).toBeInTheDocument()
  })

  it('renders failed assertion diff from error detail', () => {
    render(<ExecutionDetailPanel nodeId="check" execution={FAILED_ASSERT_EXECUTION} />)
    expect(screen.getByTestId('execution-failure')).toHaveTextContent('ASSERT_STATUS_MISMATCH')
    const diff = screen.getByTestId('assertion-diff')
    expect(diff).toHaveTextContent('201')
    expect(diff).toHaveTextContent('200')
    expect(screen.getByTestId('assertion-$.status')).toHaveClass('assertion-list__item--failed')
  })
})
