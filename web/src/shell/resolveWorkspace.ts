import { ASYNC_POLL_FAIL_RUN, ASYNC_POLL_FAIL_SCENARIO } from '../data/fixtures/asyncPollFail'
import { PARALLEL_DAG } from '../data/fixtures/parallel'
import { SERIAL_DAG, SERIAL_EVENTS, SERIAL_EXECUTIONS, SERIAL_RUN } from '../data/fixtures/serial'
import type { RunFocusScenario } from '../focus/useFailureFocus'
import type { NavCatalog, ReportNavItem } from '../nav/types'
import type { WorkspaceTab } from './workspaceTabs'

const EMPTY_EXECUTIONS: RunFocusScenario['executions'] = []

const SCENARIO_WORKSPACES: Record<string, RunFocusScenario> = {
  serial: {
    dag: SERIAL_DAG,
    events: SERIAL_EVENTS,
    executions: SERIAL_EXECUTIONS,
    run: SERIAL_RUN,
  },
  parallel: {
    dag: PARALLEL_DAG,
    events: [],
    executions: EMPTY_EXECUTIONS,
    run: {
      id: 'run-fixture-parallel',
      dag_id: 'parallel',
      dag_version: 1,
      env_name: 'local',
      mode: 'MANUAL',
      status: 'COMPLETED',
      started_at: null,
      finished_at: null,
    },
  },
  'async-poll': ASYNC_POLL_FAIL_SCENARIO,
}

function reportById(catalog: NavCatalog, reportId: string): ReportNavItem | undefined {
  return catalog.reports.find((item) => item.id === reportId)
}

function scenarioIdForReport(report: ReportNavItem): string {
  if (report.id === SERIAL_RUN.id) {
    return 'serial'
  }
  if (report.id === ASYNC_POLL_FAIL_RUN.id) {
    return 'async-poll'
  }
  return 'serial'
}

export function resolveWorkspaceForTab(
  tab: WorkspaceTab,
  catalog: NavCatalog,
): RunFocusScenario | null {
  if (tab.kind === 'scenario') {
    return SCENARIO_WORKSPACES[tab.refId] ?? null
  }
  const report = reportById(catalog, tab.refId)
  if (!report) {
    return null
  }
  const scenarioId = scenarioIdForReport(report)
  return SCENARIO_WORKSPACES[scenarioId] ?? null
}
