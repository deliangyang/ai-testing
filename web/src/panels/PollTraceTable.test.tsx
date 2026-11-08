import { render, screen } from '@testing-library/react'

import { POLL_TIMEOUT_EXECUTION } from '../data/fixtures/pollTimeout'
import { flattenPollTraces } from './pollTrace'
import { PollTraceTable } from './PollTraceTable'

describe('PollTraceTable', () => {
  it('renders all timeout poll attempts', () => {
    const rows = flattenPollTraces([POLL_TIMEOUT_EXECUTION])
    render(<PollTraceTable rows={rows} />)
    expect(screen.getByTestId('poll-trace-table')).toBeInTheDocument()
    for (let attempt = 1; attempt <= 4; attempt += 1) {
      expect(screen.getByTestId(`poll-trace-row-wait_paid-${String(attempt)}`)).toHaveTextContent(
        'PENDING',
      )
    }
    expect(screen.getAllByRole('row')).toHaveLength(5)
  })
})
