import type { Condition, Dag } from '../../types/generated'
import type {
  DagDetailDto,
  DagVersionDto,
  NodeExecutionDto,
  RunDto,
  StoredRunEvent,
} from '../types'

/** Mirrors `examples/serial.yaml` plus synthetic run evidence for UI development. */
export const SERIAL_DAG: Dag = {
  id: 'serial',
  nodes: [
    { id: 'create', type: 'http', method: 'POST', path: '/orders' },
    {
      id: 'pay',
      type: 'http',
      method: 'POST',
      path: '/orders/${create.output.body.id}/pay',
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
    { from: 'pay', to: 'check' },
  ],
}

const createdAt = '2026-11-08T14:00:00.000+08:00'

export const SERIAL_DAG_ACTIVE: DagDetailDto = {
  id: 'serial',
  version: 1,
  status: 'ACTIVE',
  created_by: 'fixture',
  created_at: createdAt,
  dag: SERIAL_DAG,
}

export const SERIAL_DAG_VERSIONS: DagVersionDto[] = [
  {
    id: 'serial',
    version: 1,
    status: 'ACTIVE',
    created_by: 'fixture',
    created_at: createdAt,
  },
]

export const SERIAL_RUN: RunDto = {
  id: 'run-fixture-serial',
  dag_id: 'serial',
  dag_version: 1,
  env_name: 'local',
  mode: 'MANUAL',
  status: 'COMPLETED',
  started_at: '2026-11-08T14:01:00.000+08:00',
  finished_at: '2026-11-08T14:01:02.500+08:00',
}

export const SERIAL_EXECUTIONS: NodeExecutionDto[] = [
  {
    run_id: 'run-fixture-serial',
    node_id: 'create',
    attempt: 1,
    status: 'COMPLETED',
    input_resolved: { method: 'POST', path: '/orders' },
    output: { status: 200, body: { id: 'o-fixture' } },
    poll_trace: null,
    failure: null,
    started_at: '2026-11-08T14:01:00.100+08:00',
    finished_at: '2026-11-08T14:01:00.400+08:00',
  },
  {
    run_id: 'run-fixture-serial',
    node_id: 'pay',
    attempt: 1,
    status: 'COMPLETED',
    input_resolved: {
      method: 'POST',
      path: '/orders/o-fixture/pay',
    },
    output: { status: 200, body: { paid: true } },
    poll_trace: null,
    failure: null,
    started_at: '2026-11-08T14:01:00.500+08:00',
    finished_at: '2026-11-08T14:01:01.200+08:00',
  },
  {
    run_id: 'run-fixture-serial',
    node_id: 'check',
    attempt: 1,
    status: 'COMPLETED',
    input_resolved: null,
    output: { results: [{ path: '$.status', passed: true }] },
    poll_trace: null,
    failure: null,
    started_at: '2026-11-08T14:01:01.300+08:00',
    finished_at: '2026-11-08T14:01:01.400+08:00',
  },
]

export const SERIAL_EVENTS: StoredRunEvent[] = [
  {
    seq: 1,
    event: { type: 'node_state_changed', node: 'create', status: 'RUNNING' },
  },
  {
    seq: 2,
    event: { type: 'node_state_changed', node: 'create', status: 'COMPLETED' },
  },
  {
    seq: 3,
    event: { type: 'node_state_changed', node: 'pay', status: 'RUNNING' },
  },
  {
    seq: 4,
    event: { type: 'node_state_changed', node: 'pay', status: 'COMPLETED' },
  },
  {
    seq: 5,
    event: { type: 'node_state_changed', node: 'check', status: 'RUNNING' },
  },
  {
    seq: 6,
    event: { type: 'node_state_changed', node: 'check', status: 'COMPLETED' },
  },
  { seq: 7, event: { type: 'run_finished', status: 'COMPLETED' } },
]
