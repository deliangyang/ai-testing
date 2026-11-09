import { describe, expect, it } from 'vitest'

import { SERIAL_DAG } from '../data/fixtures/serial'
import { applyCopilotAction } from './applyActions'

describe('applyCopilotAction', () => {
  it('inserts node and edge on canvas', () => {
    const result = applyCopilotAction(SERIAL_DAG, {
      type: 'insert_canvas',
      node: { id: 'health', type: 'http', method: 'GET', path: '/health' },
      edges: [{ from: 'create', to: 'health' }],
    })
    expect(result.dag.nodes.some((n) => n.id === 'health')).toBe(true)
    expect(result.dag.edges?.some((e) => e.from === 'create' && e.to === 'health')).toBe(true)
    expect(result.focusNodeId).toBe('health')
  })

  it('adopts assertion on target node', () => {
    const result = applyCopilotAction(SERIAL_DAG, {
      type: 'adopt_assertion',
      node_id: 'check',
      assertions: [
        { path: '$.body.paid', op: 'eq', expected: true as unknown as { [k: string]: unknown } },
      ],
    })
    const check = result.dag.nodes.find((n) => n.id === 'check' && n.type === 'assert')
    expect(check?.type).toBe('assert')
    if (check?.type === 'assert') {
      expect(check.assertions.some((a) => a.path === '$.body.paid')).toBe(true)
    }
  })
})
