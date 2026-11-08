import type { Condition, Dag } from '../../types/generated'

/** Mirrors `examples/parallel.yaml`. */
export const PARALLEL_DAG: Dag = {
  id: 'parallel',
  nodes: [
    { id: 'create', type: 'http', method: 'POST', path: '/orders' },
    { id: 'stock', type: 'http', method: 'GET', path: '/stock/A-1' },
    { id: 'price', type: 'http', method: 'GET', path: '/price/A-1' },
    {
      id: 'check',
      type: 'assert',
      target: 'stock',
      assertions: [
        { path: '$.status', op: 'eq', expected: 200 } as unknown as Condition,
        {
          path: '$.status',
          op: 'eq',
          expected: '${price.output.status}',
        } as unknown as Condition,
      ],
    },
  ],
  edges: [
    { from: 'create', to: 'stock' },
    { from: 'create', to: 'price' },
    { from: 'stock', to: 'check' },
    { from: 'price', to: 'check' },
  ],
}
