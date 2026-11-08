import { useMemo } from 'react'

import type { Dag } from '../types/generated'
import type { NodeExecutionDto, StoredRunEvent } from '../data/types'
import { flattenPollTraces } from '../panels/pollTrace'
import {
  extractRunLogEntries,
  filterRunLogEntries,
  findFailedNodeId,
  type RunLogEntry,
} from './failureFocus'

export interface RunFocusScenario {
  dag: Dag
  events: StoredRunEvent[]
  executions: NodeExecutionDto[]
}

export interface FailureFocusState {
  failedNodeId: string | null
  execution: NodeExecutionDto | null
  allLogs: RunLogEntry[]
  filteredLogs: RunLogEntry[]
  pollRows: ReturnType<typeof flattenPollTraces>
  executions: NodeExecutionDto[]
  dag: Dag
  events: StoredRunEvent[]
}

export function useFailureFocus(scenario: RunFocusScenario): FailureFocusState {
  return useMemo(() => {
    const failedNodeId = findFailedNodeId(scenario.dag, scenario.executions)
    const execution = failedNodeId
      ? (scenario.executions.find((row) => row.node_id === failedNodeId) ?? null)
      : null
    const allLogs = extractRunLogEntries(scenario.events)
    const filteredLogs = filterRunLogEntries(allLogs, failedNodeId)
    const pollRows = flattenPollTraces(
      failedNodeId
        ? scenario.executions.filter((row) => row.node_id === failedNodeId)
        : scenario.executions,
    )
    return {
      failedNodeId,
      execution,
      allLogs,
      filteredLogs,
      pollRows,
      executions: scenario.executions,
      dag: scenario.dag,
      events: scenario.events,
    }
  }, [scenario])
}
