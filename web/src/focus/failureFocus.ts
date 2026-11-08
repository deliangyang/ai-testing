import type { Dag } from '../types/generated'
import type { NodeExecutionDto, StoredRunEvent } from '../data/types'

function sortedNodeIds(dag: Dag): string[] {
  return [...new Set(dag.nodes.map((n) => n.id))].sort()
}

/** First failed node in DAG order (deterministic). */
export function findFailedNodeId(dag: Dag, executions: NodeExecutionDto[]): string | null {
  const failed = new Set(
    executions.filter((row) => row.status === 'FAILED').map((row) => row.node_id),
  )
  if (failed.size === 0) {
    return null
  }
  for (const id of sortedNodeIds(dag)) {
    if (failed.has(id)) {
      return id
    }
  }
  return [...failed].sort()[0] ?? null
}

export interface RunLogEntry {
  seq: number
  node?: string
  level: string
  message: string
}

export function extractRunLogEntries(events: StoredRunEvent[]): RunLogEntry[] {
  return [...events]
    .sort((a, b) => a.seq - b.seq)
    .flatMap((item) => {
      if (item.event.type !== 'log') {
        return []
      }
      return [
        {
          seq: item.seq,
          node: item.event.node,
          level: item.event.level,
          message: item.event.message,
        },
      ]
    })
}

/** Keep logs for the focused node; unscoped logs remain visible. */
export function filterRunLogEntries(
  entries: RunLogEntry[],
  focusNodeId: string | null,
): RunLogEntry[] {
  if (!focusNodeId) {
    return entries
  }
  return entries.filter((entry) => entry.node === undefined || entry.node === focusNodeId)
}
