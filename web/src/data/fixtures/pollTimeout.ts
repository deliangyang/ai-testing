import type { NodeExecutionDto } from '../types'

/** Timed-out poll node with a full multi-attempt trace (mirrors CLI timeout run evidence). */
export const POLL_TIMEOUT_EXECUTION: NodeExecutionDto = {
  run_id: 'run-fixture-poll-timeout',
  node_id: 'wait_paid',
  attempt: 1,
  status: 'FAILED',
  input_resolved: {
    method: 'GET',
    path: '/orders/o-timeout',
  },
  output: {
    kind: 'timeout',
    message: 'poll deadline exceeded',
    detail: {
      reason: 'deadline',
      attempts: 4,
      timeout_ms: 5000,
    },
  },
  poll_trace: [
    {
      ts: '2026-11-08T14:05:00.100+08:00',
      attempt: 1,
      passed: false,
      actual: 'PENDING',
      expected: 'PAID',
      diff: 'actual PENDING, expected PAID',
    },
    {
      ts: '2026-11-08T14:05:00.120+08:00',
      attempt: 2,
      passed: false,
      actual: 'PENDING',
      expected: 'PAID',
      diff: 'actual PENDING, expected PAID',
    },
    {
      ts: '2026-11-08T14:05:00.150+08:00',
      attempt: 3,
      passed: false,
      actual: 'PENDING',
      expected: 'PAID',
      diff: 'actual PENDING, expected PAID',
    },
    {
      ts: '2026-11-08T14:05:00.200+08:00',
      attempt: 4,
      passed: false,
      actual: 'PENDING',
      expected: 'PAID',
      diff: 'actual PENDING, expected PAID',
    },
  ],
  failure: {
    category: 'timeout',
    error_code: 'POLL_DEADLINE',
  },
  started_at: '2026-11-08T14:05:00.000+08:00',
  finished_at: '2026-11-08T14:05:00.250+08:00',
}
