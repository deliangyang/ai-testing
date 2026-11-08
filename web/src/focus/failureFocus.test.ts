import { ASYNC_POLL_FAIL_SCENARIO } from '../data/fixtures/asyncPollFail'
import {
  extractRunLogEntries,
  filterRunLogEntries,
  findFailedNodeId,
} from './failureFocus'

describe('failureFocus', () => {
  it('picks wait_paid as the first failed node', () => {
    expect(findFailedNodeId(ASYNC_POLL_FAIL_SCENARIO.dag, ASYNC_POLL_FAIL_SCENARIO.executions)).toBe(
      'wait_paid',
    )
  })

  it('filters drawer logs to the failed node', () => {
    const logs = extractRunLogEntries(ASYNC_POLL_FAIL_SCENARIO.events)
    const filtered = filterRunLogEntries(logs, 'wait_paid')
    expect(filtered).toHaveLength(1)
    expect(filtered[0]?.message).toBe('poll deadline exceeded')
    expect(logs.length).toBeGreaterThan(filtered.length)
  })
})
