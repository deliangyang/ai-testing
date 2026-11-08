import type { NodeExecutionDto } from '../data/types'
import { formatDisplayValue } from './formatValue'

export interface PollTraceEntry {
  attempt: number
  ts?: string
  passed: boolean
  actual?: unknown
  expected?: unknown
  diff?: string
  error?: unknown
}

export interface PollTraceTableRow {
  nodeId: string
  attempt: number
  timestamp: string
  actual: string
  distance: string
  passed: boolean
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

export function parsePollTrace(value: unknown): PollTraceEntry[] {
  if (!Array.isArray(value)) {
    return []
  }
  const entries: PollTraceEntry[] = []
  for (const item of value) {
    if (!isRecord(item) || typeof item.attempt !== 'number') {
      continue
    }
    entries.push({
      attempt: item.attempt,
      ts: typeof item.ts === 'string' ? item.ts : undefined,
      passed: item.passed === true,
      actual: item.actual,
      expected: item.expected,
      diff: typeof item.diff === 'string' ? item.diff : undefined,
      error: item.error,
    })
  }
  return entries.sort((a, b) => a.attempt - b.attempt)
}

export function pollDistanceLabel(entry: PollTraceEntry): string {
  if (entry.diff && entry.diff.length > 0) {
    return entry.diff
  }
  if (entry.error !== undefined && entry.error !== null) {
    return formatDisplayValue(entry.error)
  }
  const actual = formatDisplayValue(entry.actual)
  const expected = formatDisplayValue(entry.expected)
  if (actual === '—' && expected === '—') {
    return '—'
  }
  return `actual ${actual}, expected ${expected}`
}

export function flattenPollTraces(executions: NodeExecutionDto[]): PollTraceTableRow[] {
  const rows: PollTraceTableRow[] = []
  const sortedExecs = [...executions].sort((a, b) => a.node_id.localeCompare(b.node_id))
  for (const exec of sortedExecs) {
    for (const entry of parsePollTrace(exec.poll_trace)) {
      rows.push({
        nodeId: exec.node_id,
        attempt: entry.attempt,
        timestamp: entry.ts ?? '—',
        actual: formatDisplayValue(entry.actual),
        distance: pollDistanceLabel(entry),
        passed: entry.passed,
      })
    }
  }
  return rows.sort((a, b) => {
    const n = a.nodeId.localeCompare(b.nodeId)
    if (n !== 0) {
      return n
    }
    return a.attempt - b.attempt
  })
}
