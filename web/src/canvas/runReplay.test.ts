import { ASYNC_POLL_DAG } from '../data/fixtures/asyncPoll'
import { ASYNC_POLL_RUN_EVENTS } from '../data/fixtures/asyncPollRun'
import { PARALLEL_DAG } from '../data/fixtures/parallel'
import { PARALLEL_RUN_EVENTS } from '../data/fixtures/parallelRun'
import { SERIAL_DAG } from '../data/fixtures/serial'
import { SERIAL_EVENTS } from '../data/fixtures/serial'
import { applyRunEvents, formatPollStatusLine, runProgressLabel } from './runReplay'

describe('applyRunEvents', () => {
  it('replays serial fixture to completion', () => {
    const view = applyRunEvents(SERIAL_DAG, SERIAL_EVENTS)
    expect(view.runStatus).toBe('COMPLETED')
    expect(view.progress).toEqual({ done: 3, total: 3 })
    for (const id of ['create', 'pay', 'check']) {
      expect(view.nodeStates[id].status).toBe('COMPLETED')
    }
  })

  it('shows mid-run RUNNING from partial replay', () => {
    const view = applyRunEvents(SERIAL_DAG, SERIAL_EVENTS, 3)
    expect(view.nodeStates.create.status).toBe('COMPLETED')
    expect(view.nodeStates.pay.status).toBe('RUNNING')
    expect(view.progress.done).toBe(1)
  })

  it('keeps parallel branches RUNNING together', () => {
    const view = applyRunEvents(PARALLEL_DAG, PARALLEL_RUN_EVENTS, 4)
    expect(view.nodeStates.stock.status).toBe('RUNNING')
    expect(view.nodeStates.price.status).toBe('RUNNING')
  })

  it('formats poll iteration line from async-poll events', () => {
    const view = applyRunEvents(ASYNC_POLL_DAG, ASYNC_POLL_RUN_EVENTS, 6)
    const line = formatPollStatusLine(view.nodeStates.wait_paid)
    expect(line).toMatch(/第 2 次/)
    expect(line).toMatch(/状态=PENDING/)
  })
})

describe('runProgressLabel', () => {
  it('summarizes in-flight runs', () => {
    expect(runProgressLabel({ done: 1, total: 4 })).toBe('1/4 节点 · 执行中')
  })
})
