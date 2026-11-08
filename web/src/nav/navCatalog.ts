import { ASYNC_POLL_FAIL_RUN } from '../data/fixtures/asyncPollFail'
import { SERIAL_RUN } from '../data/fixtures/serial'
import type { NavCatalog } from './types'

/** Fixture catalog aligned with read API list payloads (P1-03). */
export const FIXTURE_NAV_CATALOG: NavCatalog = {
  scenarios: [
    {
      id: 'serial',
      title: 'serial',
      lastRunStatus: 'COMPLETED',
      passRate: 1,
    },
    {
      id: 'parallel',
      title: 'parallel',
      lastRunStatus: 'COMPLETED',
      passRate: 0.92,
    },
    {
      id: 'async-poll',
      title: 'async-poll',
      lastRunStatus: 'FAILED',
      passRate: 0.75,
    },
  ],
  plans: [
    { id: 'serial-active', title: 'serial · v1', gateStatus: 'pass' },
    { id: 'parallel-draft', title: 'parallel · pending', gateStatus: 'warn' },
    { id: 'broken-import', title: 'import sketch', gateStatus: 'fail' },
  ],
  reports: [
    {
      id: SERIAL_RUN.id,
      title: 'serial · manual',
      runStatus: SERIAL_RUN.status,
      finishedAt: SERIAL_RUN.finished_at,
    },
    {
      id: ASYNC_POLL_FAIL_RUN.id,
      title: 'async-poll · timeout',
      runStatus: ASYNC_POLL_FAIL_RUN.status,
      finishedAt: ASYNC_POLL_FAIL_RUN.finished_at,
    },
  ],
}
