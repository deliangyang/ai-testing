import { fireEvent, render, screen } from '@testing-library/react'

import { SERIAL_DAG, SERIAL_EVENTS, SERIAL_RUN } from '../data/fixtures/serial'
import { DagReportReplayView } from './DagReportReplayView'

describe('DagReportReplayView', () => {
  it('scrubs timeline and updates running state', () => {
    render(<DagReportReplayView dag={SERIAL_DAG} events={SERIAL_EVENTS} run={SERIAL_RUN} />)
    expect(screen.getByTestId('run-report-summary')).toBeInTheDocument()
    fireEvent.change(screen.getByTestId('run-replay-seq'), { target: { value: '3' } })
    expect(screen.getByTestId('run-replay-seq-label')).toHaveTextContent('3/7')
    expect(screen.getByTestId('dag-run-node-pay')).toHaveAttribute('data-status', 'RUNNING')
  })
})
