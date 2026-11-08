import { render, screen } from '@testing-library/react'

import { ASYNC_POLL_FAIL_SCENARIO } from '../data/fixtures/asyncPollFail'
import { StatusBar } from './StatusBar'
import { buildStatusBarModel } from './statusBarModel'

describe('StatusBar UI', () => {
  it('renders connection, execution, env and elapsed segments', () => {
    const model = buildStatusBarModel({
      dag: ASYNC_POLL_FAIL_SCENARIO.dag,
      events: ASYNC_POLL_FAIL_SCENARIO.events,
      run: ASYNC_POLL_FAIL_SCENARIO.run,
      connections: { sqliteConnected: true, sseConnected: true },
    })
    render(<StatusBar model={model} />)
    expect(screen.getByTestId('status-storage')).toHaveTextContent('SQLite 已连接')
    expect(screen.getByTestId('status-execution')).toHaveTextContent('4/4 节点')
    expect(screen.getByTestId('status-sse')).toHaveTextContent('SSE 已连接')
    expect(screen.getByTestId('status-env')).toHaveTextContent('local')
    expect(screen.getByTestId('status-elapsed')).toHaveTextContent('250 ms')
    expect(screen.getByTestId('status-shortcut-hint')).toHaveTextContent('Ctrl+K')
  })
})
