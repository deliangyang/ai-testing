import { SERIAL_EXECUTIONS } from '../data/fixtures/serial'
import { formatTimingBreakdown, timingBreakdown } from './executionTiming'

describe('timingBreakdown', () => {
  it('computes wall and request duration for pay fixture', () => {
    const pay = SERIAL_EXECUTIONS.find((e) => e.node_id === 'pay')
    expect(pay).toBeDefined()
    if (!pay) {
      return
    }
    const timing = timingBreakdown(pay)
    expect(timing.wallMs).toBe(700)
    expect(timing.requestMs).toBe(42)
    const lines = formatTimingBreakdown(timing)
    expect(lines).toContain('节点总耗时 700 ms')
    expect(lines).toContain('请求耗时 42 ms')
  })
})
