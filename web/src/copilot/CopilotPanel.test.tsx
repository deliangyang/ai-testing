import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { FixtureDataSource } from '../data/fixture'
import { SERIAL_DAG } from '../data/fixtures/serial'
import { CopilotPanel } from './CopilotPanel'

describe('CopilotPanel', () => {
  it('shows context strip and fixture assistant actions', async () => {
    const dataSource = new FixtureDataSource('serial')
    render(
      <CopilotPanel
        dag={SERIAL_DAG}
        selectedNodeId="pay"
        dataSource={dataSource}
        onDagChange={() => undefined}
        onFocusNode={() => undefined}
        onSelectNode={() => undefined}
      />,
    )
    expect(screen.getByTestId('copilot-context-dag')).toHaveTextContent('serial')
    expect(screen.getByTestId('copilot-context-node')).toHaveTextContent('pay')
    fireEvent.click(screen.getByTestId('copilot-send'))
    await screen.findByTestId('copilot-action-insert_canvas-0')
    expect(screen.getByTestId('copilot-action-focus_node-1')).toBeInTheDocument()
  })
})
