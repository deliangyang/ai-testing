import type { NodeExecutionDto } from '../data/types'
import { formatDisplayValue } from '../panels/formatValue'

export interface WatchedVariable {
  name: string
  value: string
  source: string
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

export function watchVariablesFromExecutions(executions: NodeExecutionDto[]): WatchedVariable[] {
  const rows: WatchedVariable[] = []
  const sorted = [...executions].sort((a, b) => a.node_id.localeCompare(b.node_id))
  for (const exec of sorted) {
    if (exec.status !== 'COMPLETED' || exec.output === null || !isRecord(exec.output)) {
      continue
    }
    const prefix = exec.node_id
    if (typeof exec.output.status === 'number') {
      rows.push({
        name: `${prefix}.output.status`,
        value: String(exec.output.status),
        source: exec.node_id,
      })
    }
    if (isRecord(exec.output.body)) {
      for (const [key, value] of Object.entries(exec.output.body).sort(([a], [b]) =>
        a.localeCompare(b),
      )) {
        rows.push({
          name: `${prefix}.output.body.${key}`,
          value: formatDisplayValue(value),
          source: exec.node_id,
        })
      }
    }
  }
  return rows.sort((a, b) => a.name.localeCompare(b.name))
}

export function filterWatchVariables(
  variables: WatchedVariable[],
  text: string,
): WatchedVariable[] {
  const q = text.trim().toLowerCase()
  if (q.length === 0) {
    return variables
  }
  return variables.filter(
    (row) =>
      row.name.toLowerCase().includes(q) ||
      row.value.toLowerCase().includes(q) ||
      row.source.toLowerCase().includes(q),
  )
}
