import type { StoredRunEvent } from '../types'

/** Overlapping stock/price RUNNING for parallel pulse UI tests. */
export const PARALLEL_RUN_EVENTS: StoredRunEvent[] = [
  { seq: 1, event: { type: 'node_state_changed', node: 'create', status: 'RUNNING' } },
  { seq: 2, event: { type: 'node_state_changed', node: 'create', status: 'COMPLETED' } },
  { seq: 3, event: { type: 'node_state_changed', node: 'stock', status: 'RUNNING' } },
  { seq: 4, event: { type: 'node_state_changed', node: 'price', status: 'RUNNING' } },
  { seq: 5, event: { type: 'node_state_changed', node: 'stock', status: 'COMPLETED' } },
  { seq: 6, event: { type: 'node_state_changed', node: 'price', status: 'COMPLETED' } },
  { seq: 7, event: { type: 'node_state_changed', node: 'check', status: 'RUNNING' } },
  { seq: 8, event: { type: 'node_state_changed', node: 'check', status: 'COMPLETED' } },
  { seq: 9, event: { type: 'run_finished', status: 'COMPLETED' } },
]
