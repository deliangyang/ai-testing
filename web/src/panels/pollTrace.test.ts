import { POLL_TIMEOUT_EXECUTION } from '../data/fixtures/pollTimeout'
import { flattenPollTraces, parsePollTrace } from './pollTrace'

describe('pollTrace', () => {
  it('parses full timeout fixture trace in attempt order', () => {
    const entries = parsePollTrace(POLL_TIMEOUT_EXECUTION.poll_trace)
    expect(entries).toHaveLength(4)
    expect(entries.map((e) => e.attempt)).toEqual([1, 2, 3, 4])
    expect(entries.every((e) => e.actual === 'PENDING')).toBe(true)
  })

  it('flattens table rows with distance column', () => {
    const rows = flattenPollTraces([POLL_TIMEOUT_EXECUTION])
    expect(rows).toHaveLength(4)
    expect(rows[0]).toMatchObject({
      nodeId: 'wait_paid',
      attempt: 1,
      actual: 'PENDING',
    })
    expect(rows[3]?.distance).toContain('expected PAID')
  })
})
