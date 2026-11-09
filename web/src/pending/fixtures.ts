import type { Dag } from '../types/generated'
import type { DagDetailDto, DagVersionDto } from '../data/types'
import { SERIAL_DAG } from '../data/fixtures/serial'

/** AI 候选：在 serial 上增加只读库存查询节点。 */
export const SERIAL_AI_CANDIDATE_DAG: Dag = {
  id: 'serial',
  nodes: [
    ...SERIAL_DAG.nodes,
    { id: 'lookup_stock', type: 'http', method: 'GET', path: '/stock/sku-1' },
  ],
  edges: [...(SERIAL_DAG.edges ?? []), { from: 'create', to: 'lookup_stock' }],
}

const pendingAt = '2026-11-09T10:00:00.000+08:00'

export const SERIAL_AI_PENDING_VERSION: DagVersionDto = {
  id: 'serial',
  version: 2,
  status: 'PENDING',
  created_by: 'ai',
  created_at: pendingAt,
}

export const SERIAL_AI_PENDING_DETAIL: DagDetailDto = {
  ...SERIAL_AI_PENDING_VERSION,
  dag: SERIAL_AI_CANDIDATE_DAG,
}
