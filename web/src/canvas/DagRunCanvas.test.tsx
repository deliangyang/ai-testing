import { render, screen } from '@testing-library/react'

import { ASYNC_POLL_DAG } from '../data/fixtures/asyncPoll'
import { ASYNC_POLL_RUN_EVENTS } from '../data/fixtures/asyncPollRun'
import { PARALLEL_DAG } from '../data/fixtures/parallel'
import { PARALLEL_RUN_EVENTS } from '../data/fixtures/parallelRun'
import { SERIAL_DAG } from '../data/fixtures/serial'
import { SERIAL_EVENTS } from '../data/fixtures/serial'
import { DagRunCanvas } from './DagRunCanvas'

describe('DagRunCanvas', () => {
  it('replays serial events with completed nodes', () => {
    render(<DagRunCanvas dag={SERIAL_DAG} events={SERIAL_EVENTS} />)
    expect(screen.getByTestId('run-progress-label')).toHaveTextContent('3/3 节点 · 已完成')
    for (const id of ['create', 'pay', 'check']) {
      expect(screen.getByTestId(`dag-run-node-${id}`)).toHaveAttribute('data-status', 'COMPLETED')
    }
  })

  it('pulses parallel RUNNING branches at overlapping replay', () => {
    render(<DagRunCanvas dag={PARALLEL_DAG} events={PARALLEL_RUN_EVENTS} replaySeq={4} />)
    const stock = screen.getByTestId('dag-run-node-stock')
    const price = screen.getByTestId('dag-run-node-price')
    expect(stock).toHaveClass('dag-node-card--pulse')
    expect(price).toHaveClass('dag-node-card--pulse')
    expect(stock).toHaveAttribute('data-status', 'RUNNING')
    expect(price).toHaveAttribute('data-status', 'RUNNING')
  })

  it('shows poll status line during async-poll replay', () => {
    render(<DagRunCanvas dag={ASYNC_POLL_DAG} events={ASYNC_POLL_RUN_EVENTS} replaySeq={6} />)
    expect(screen.getByTestId('dag-poll-line-wait_paid')).toHaveTextContent(/第 2 次/)
    expect(screen.getByTestId('dag-poll-line-wait_paid')).toHaveTextContent(/状态=PENDING/)
  })
})
