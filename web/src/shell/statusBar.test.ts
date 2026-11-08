import { ASYNC_POLL_FAIL_SCENARIO } from '../data/fixtures/asyncPollFail'
import {
  buildStatusBarModel,
  formatElapsed,
  formatExecutionSummary,
} from './statusBarModel'

describe('statusBar', () => {
  it('formats elapsed time from run timestamps', () => {
    expect(formatElapsed(ASYNC_POLL_FAIL_SCENARIO.run)).toBe('250 ms')
  })

  it('builds summary with polling while replay is in-flight', () => {
    const mid = buildStatusBarModel({
      dag: ASYNC_POLL_FAIL_SCENARIO.dag,
      events: ASYNC_POLL_FAIL_SCENARIO.events,
      run: ASYNC_POLL_FAIL_SCENARIO.run,
      connections: { sqliteConnected: true, sseConnected: true },
      parallel: 2,
      replaySeq: 7,
    })
    expect(mid.executionSummary).toBe('2/4 节点 · 并行 2 · 1 轮询中')
    expect(mid.storageLabel).toBe('SQLite 已连接')
    expect(mid.elapsedLabel).toBe('耗时 250 ms')
  })

  it('omits polling segment when no nodes are polling', () => {
    const done = buildStatusBarModel({
      dag: ASYNC_POLL_FAIL_SCENARIO.dag,
      events: ASYNC_POLL_FAIL_SCENARIO.events,
      run: ASYNC_POLL_FAIL_SCENARIO.run,
      connections: { sqliteConnected: true, sseConnected: false },
    })
    expect(done.executionSummary).toBe('4/4 节点 · 并行 1')
    expect(done.sseLabel).toBe('SSE 断开')
  })

  it('formats execution summary helper', () => {
    expect(formatExecutionSummary({ done: 3, total: 12 }, 2, 1)).toBe(
      '3/12 节点 · 并行 2 · 1 轮询中',
    )
  })
})
