import type { NodeExecutionDto } from '../types'

/** Assert node FAILED with assertion diff in error detail (matches engine/store shape). */
export const FAILED_ASSERT_EXECUTION: NodeExecutionDto = {
  run_id: 'run-fixture-fail',
  node_id: 'check',
  attempt: 1,
  status: 'FAILED',
  input_resolved: null,
  output: {
    kind: 'assertion',
    message: '1 of 1 assertions failed',
    detail: {
      results: [
        {
          path: '$.status',
          operator: 'eq',
          expected: 201,
          actual: 200,
          passed: false,
          message: 'values differ',
        },
      ],
    },
  },
  poll_trace: null,
  failure: {
    category: 'assertion_failed_status',
    error_code: 'ASSERT_STATUS_MISMATCH',
  },
  started_at: '2026-11-08T14:02:00.100+08:00',
  finished_at: '2026-11-08T14:02:00.250+08:00',
}
