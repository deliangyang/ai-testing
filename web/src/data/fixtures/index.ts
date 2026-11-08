import { ASYNC_POLL_DAG } from './asyncPoll'
import { PARALLEL_DAG } from './parallel'
import {
  SERIAL_DAG,
  SERIAL_DAG_ACTIVE,
  SERIAL_DAG_VERSIONS,
  SERIAL_EVENTS,
  SERIAL_EXECUTIONS,
  SERIAL_RUN,
} from './serial'
import type {
  DagDetailDto,
  DagVersionDto,
  NodeExecutionDto,
  RunDto,
  StoredRunEvent,
} from '../types'
import type { Dag } from '../../types/generated'

export interface ScenarioFixture {
  id: string
  dagVersions: DagVersionDto[]
  activeDag: DagDetailDto
  runs: RunDto[]
  executionsByRun: Record<string, NodeExecutionDto[]>
  eventsByRun: Record<string, StoredRunEvent[]>
  /** Canonical DAG documents keyed by id (for validate/save). */
  dags: Record<string, Dag>
}

export const SCENARIOS: Record<string, ScenarioFixture> = {
  serial: {
    id: 'serial',
    dagVersions: SERIAL_DAG_VERSIONS,
    activeDag: SERIAL_DAG_ACTIVE,
    runs: [SERIAL_RUN],
    executionsByRun: { [SERIAL_RUN.id]: SERIAL_EXECUTIONS },
    eventsByRun: { [SERIAL_RUN.id]: SERIAL_EVENTS },
    dags: { serial: SERIAL_DAG },
  },
  parallel: {
    id: 'parallel',
    dagVersions: [],
    activeDag: {
      id: 'parallel',
      version: 1,
      status: 'ACTIVE',
      created_by: 'fixture',
      created_at: '2026-11-08T14:00:00.000+08:00',
      dag: PARALLEL_DAG,
    },
    runs: [],
    executionsByRun: {},
    eventsByRun: {},
    dags: { parallel: PARALLEL_DAG },
  },
  'async-poll': {
    id: 'async-poll',
    dagVersions: [],
    activeDag: {
      id: 'async-poll',
      version: 1,
      status: 'ACTIVE',
      created_by: 'fixture',
      created_at: '2026-11-08T14:00:00.000+08:00',
      dag: ASYNC_POLL_DAG,
    },
    runs: [],
    executionsByRun: {},
    eventsByRun: {},
    dags: { 'async-poll': ASYNC_POLL_DAG },
  },
}

export const DEFAULT_SCENARIO = 'serial'
