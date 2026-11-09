import { describe, expect, it } from 'vitest'

import { SERIAL_DAG } from '../data/fixtures/serial'
import { SERIAL_AI_CANDIDATE_DAG } from './fixtures'
import { diffDags } from './dagDiff'

describe('diffDags', () => {
  it('列出新增节点与边', () => {
    const rows = diffDags(SERIAL_DAG, SERIAL_AI_CANDIDATE_DAG)
    expect(rows.some((r) => r.kind === 'added' && r.node_id === 'lookup_stock')).toBe(true)
    expect(rows.some((r) => r.kind === 'added' && r.node_id.includes('create->lookup_stock'))).toBe(
      true,
    )
  })
})
