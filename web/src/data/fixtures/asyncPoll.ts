import type { Condition, Dag } from '../../types/generated'

/** Mirrors `examples/async-poll.yaml`. */
export const ASYNC_POLL_DAG: Dag = {
  id: 'async-poll',
  nodes: [
    { id: 'create', type: 'http', method: 'POST', path: '/orders' },
    {
      id: 'pay',
      type: 'http',
      method: 'POST',
      path: '/orders/${create.output.body.id}/pay',
    },
    {
      id: 'wait_paid',
      type: 'poll',
      request: { method: 'GET', path: '/orders/${create.output.body.id}' },
      condition: { path: '$.body.status', op: 'eq', expected: 'PAID' } as unknown as Condition,
      backoff_ms: [10, 20],
      timeout_ms: 5000,
    },
    {
      id: 'check',
      type: 'assert',
      target: 'pay',
      assertions: [{ path: '$.status', op: 'eq', expected: 200 } as unknown as Condition],
    },
  ],
  edges: [
    { from: 'create', to: 'pay' },
    { from: 'pay', to: 'wait_paid' },
    { from: 'pay', to: 'check' },
  ],
}
