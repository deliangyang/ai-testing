import { render, screen } from '@testing-library/react'

import { POLL_TIMEOUT_EXECUTION } from '../data/fixtures/pollTimeout'
import { parsePollTrace } from './pollTrace'
import { PollTraceTimeline } from './PollTraceTimeline'

describe('PollTraceTimeline', () => {
  it('lists each poll attempt for the node', () => {
    const entries = parsePollTrace(POLL_TIMEOUT_EXECUTION.poll_trace)
    render(<PollTraceTimeline nodeId="wait_paid" entries={entries} />)
    expect(screen.getByTestId('poll-trace-timeline')).toBeInTheDocument()
    expect(screen.getByTestId('poll-timeline-wait_paid-4')).toHaveTextContent('第 4 次')
  })
})
