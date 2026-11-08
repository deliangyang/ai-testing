import type { NodeExecutionDto, RunDto, StoredRunEvent } from '../types'
import { ASYNC_POLL_DAG } from './asyncPoll'
import { POLL_TIMEOUT_EXECUTION } from './pollTimeout'

const createOk: NodeExecutionDto = {
  run_id: 'run-async-poll-fail',
  node_id: 'create',
  attempt: 1,
  status: 'COMPLETED',
  input_resolved: { method: 'POST', path: '/orders' },
  output: { status: 200, body: { id: 'o-timeout' } },
  poll_trace: null,
  failure: null,
  started_at: '2026-11-08T14:05:00.000+08:00',
  finished_at: '2026-11-08T14:05:00.080+08:00',
}

const payOk: NodeExecutionDto = {
  run_id: 'run-async-poll-fail',
  node_id: 'pay',
  attempt: 1,
  status: 'COMPLETED',
  input_resolved: { method: 'POST', path: '/orders/o-timeout/pay' },
  output: { status: 200, body: { paid: true } },
  poll_trace: null,
  failure: null,
  started_at: '2026-11-08T14:05:00.090+08:00',
  finished_at: '2026-11-08T14:05:00.180+08:00',
}

const waitFailed: NodeExecutionDto = {
  ...POLL_TIMEOUT_EXECUTION,
  run_id: 'run-async-poll-fail',
}

const checkSkipped: NodeExecutionDto = {
  run_id: 'run-async-poll-fail',
  node_id: 'check',
  attempt: 1,
  status: 'SKIPPED',
  input_resolved: null,
  output: null,
  poll_trace: null,
  failure: null,
  started_at: null,
  finished_at: null,
}

export const ASYNC_POLL_FAIL_EXECUTIONS: NodeExecutionDto[] = [
  createOk,
  payOk,
  waitFailed,
  checkSkipped,
]

export const ASYNC_POLL_FAIL_EVENTS: StoredRunEvent[] = [
  { seq: 1, event: { type: 'node_state_changed', node: 'create', status: 'RUNNING' } },
  { seq: 2, event: { type: 'node_state_changed', node: 'create', status: 'COMPLETED' } },
  { seq: 3, event: { type: 'log', node: 'create', level: 'info', message: 'POST /orders → 200' } },
  { seq: 4, event: { type: 'node_state_changed', node: 'pay', status: 'RUNNING' } },
  { seq: 5, event: { type: 'node_state_changed', node: 'pay', status: 'COMPLETED' } },
  { seq: 6, event: { type: 'log', node: 'pay', level: 'info', message: 'pay ok' } },
  { seq: 7, event: { type: 'node_state_changed', node: 'wait_paid', status: 'POLLING' } },
  {
    seq: 8,
    event: {
      type: 'poll_iteration',
      node: 'wait_paid',
      attempt: 1,
      entry: { actual: 'PENDING', passed: false },
    },
  },
  {
    seq: 9,
    event: {
      type: 'poll_iteration',
      node: 'wait_paid',
      attempt: 2,
      entry: { actual: 'PENDING', passed: false },
    },
  },
  { seq: 10, event: { type: 'node_state_changed', node: 'wait_paid', status: 'FAILED' } },
  {
    seq: 11,
    event: {
      type: 'log',
      node: 'wait_paid',
      level: 'error',
      message: 'poll deadline exceeded',
    },
  },
  {
    seq: 12,
    event: { type: 'log', node: 'create', level: 'info', message: 'ignored when focused' },
  },
  { seq: 13, event: { type: 'node_state_changed', node: 'check', status: 'SKIPPED' } },
  { seq: 14, event: { type: 'run_finished', status: 'FAILED' } },
]

export const ASYNC_POLL_FAIL_RUN: RunDto = {
  id: 'run-async-poll-fail',
  dag_id: 'async-poll',
  dag_version: 1,
  env_name: 'local',
  mode: 'MANUAL',
  status: 'FAILED',
  started_at: '2026-11-08T14:05:00.000+08:00',
  finished_at: '2026-11-08T14:05:00.250+08:00',
}

export const ASYNC_POLL_FAIL_SCENARIO = {
  dag: ASYNC_POLL_DAG,
  events: ASYNC_POLL_FAIL_EVENTS,
  executions: ASYNC_POLL_FAIL_EXECUTIONS,
  run: ASYNC_POLL_FAIL_RUN,
}
