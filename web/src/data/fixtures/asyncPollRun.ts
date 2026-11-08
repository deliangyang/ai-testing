import type { StoredRunEvent } from '../types'

/** Mirrors engine poll event shape (`entry.actual`). */
export const ASYNC_POLL_RUN_EVENTS: StoredRunEvent[] = [
  { seq: 1, event: { type: 'node_state_changed', node: 'create', status: 'RUNNING' } },
  { seq: 2, event: { type: 'node_state_changed', node: 'create', status: 'COMPLETED' } },
  { seq: 3, event: { type: 'node_state_changed', node: 'pay', status: 'RUNNING' } },
  { seq: 4, event: { type: 'node_state_changed', node: 'pay', status: 'COMPLETED' } },
  { seq: 5, event: { type: 'node_state_changed', node: 'wait_paid', status: 'POLLING' } },
  {
    seq: 6,
    event: {
      type: 'poll_iteration',
      node: 'wait_paid',
      attempt: 2,
      entry: { actual: 'PENDING', passed: false },
    },
  },
  {
    seq: 7,
    event: {
      type: 'poll_iteration',
      node: 'wait_paid',
      attempt: 3,
      entry: { actual: 'PAID', passed: true },
    },
  },
  { seq: 8, event: { type: 'node_state_changed', node: 'wait_paid', status: 'COMPLETED' } },
  { seq: 9, event: { type: 'node_state_changed', node: 'check', status: 'COMPLETED' } },
  { seq: 10, event: { type: 'run_finished', status: 'COMPLETED' } },
]
