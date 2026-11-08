import { render, screen } from '@testing-library/react'

import { ASYNC_POLL_DAG } from '../data/fixtures/asyncPoll'
import { PARALLEL_DAG } from '../data/fixtures/parallel'
import { SERIAL_DAG } from '../data/fixtures/serial'
import { DagEditCanvas } from './DagEditCanvas'

describe('DagEditCanvas', () => {
  it.each([
    ['serial', SERIAL_DAG, ['create', 'pay', 'check']],
    ['parallel', PARALLEL_DAG, ['create', 'stock', 'price', 'check']],
    ['async-poll', ASYNC_POLL_DAG, ['create', 'pay', 'wait_paid', 'check']],
  ] as const)('renders %s sample chain', (_name, dag, nodeIds) => {
    render(<DagEditCanvas dag={dag} />)
    expect(screen.getByTestId('dag-edit-canvas')).toBeInTheDocument()
    for (const id of nodeIds) {
      expect(screen.getByTestId(`dag-node-${id}`)).toBeInTheDocument()
    }
  })

  it('shows HTTP summaries on serial chain', () => {
    render(<DagEditCanvas dag={SERIAL_DAG} />)
    const create = screen.getByTestId('dag-node-create')
    expect(create).toHaveTextContent('POST /orders')
    expect(screen.getByTestId('dag-node-check')).toHaveTextContent('assert → pay')
  })
})
