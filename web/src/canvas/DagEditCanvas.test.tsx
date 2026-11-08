import { fireEvent, render, screen } from '@testing-library/react'
import { useState } from 'react'

import { ASYNC_POLL_DAG } from '../data/fixtures/asyncPoll'
import { PARALLEL_DAG } from '../data/fixtures/parallel'
import { SERIAL_DAG } from '../data/fixtures/serial'
import type { Dag } from '../types/generated'
import { DagEditCanvas } from './DagEditCanvas'

function Harness({ initial }: { initial: Dag }) {
  const [dag, setDag] = useState(initial)
  return <DagEditCanvas dag={dag} onDagChange={setDag} />
}

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

  it('adds an http node from the pane context menu', () => {
    render(<Harness initial={SERIAL_DAG} />)
    const canvas = screen.getByTestId('dag-edit-canvas')
    fireEvent.contextMenu(canvas.querySelector('.react-flow__pane') ?? canvas)
    fireEvent.click(screen.getByTestId('dag-edit-add-http'))
    expect(screen.getByTestId('dag-edit-canvas')).toHaveAttribute('data-node-count', '4')
    expect(screen.getByTestId('dag-node-node_1')).toBeInTheDocument()
  })

  it('deletes a node from the node context menu', () => {
    render(<Harness initial={SERIAL_DAG} />)
    fireEvent.contextMenu(screen.getByTestId('dag-node-check'))
    fireEvent.click(screen.getByTestId('dag-edit-delete-node'))
    expect(screen.getByTestId('dag-edit-canvas')).toHaveAttribute('data-node-count', '2')
    expect(screen.queryByTestId('dag-node-check')).toBeNull()
  })

  it('exposes auto layout control', () => {
    render(<DagEditCanvas dag={SERIAL_DAG} />)
    expect(screen.getByTestId('dag-edit-auto-layout')).toHaveTextContent('自动布局')
  })
})
