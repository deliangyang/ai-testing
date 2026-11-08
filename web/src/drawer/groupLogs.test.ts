import { ASYNC_POLL_FAIL_SCENARIO } from '../data/fixtures/asyncPollFail'
import { extractRunLogEntries } from '../focus/failureFocus'
import { filterLogEntries, groupLogsByNode } from './groupLogs'

describe('groupLogs', () => {
  const logs = extractRunLogEntries(ASYNC_POLL_FAIL_SCENARIO.events)

  it('groups logs by node id', () => {
    const groups = groupLogsByNode(logs)
    const ids = groups.map((g) => g.nodeId)
    expect(ids).toContain('create')
    expect(ids).toContain('pay')
    expect(ids).toContain('wait_paid')
  })

  it('filters by node and text', () => {
    const payOnly = filterLogEntries(logs, { nodeId: 'pay' })
    expect(payOnly).toHaveLength(1)
    const deadline = filterLogEntries(logs, { text: 'deadline' })
    expect(deadline.every((e) => e.message.includes('deadline'))).toBe(true)
  })
})
