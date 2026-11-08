import type { NodeExecutionDto } from '../data/types'

export interface AssertionRow {
  path: string
  passed: boolean
  operator?: string
  expected?: unknown
  actual?: unknown
  message?: string
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function rowsFromResults(value: unknown): AssertionRow[] | null {
  if (!isRecord(value) || !Array.isArray(value.results)) {
    return null
  }
  const rows: AssertionRow[] = []
  for (const item of value.results) {
    if (!isRecord(item) || typeof item.path !== 'string') {
      continue
    }
    rows.push({
      path: item.path,
      passed: item.passed === true,
      operator: typeof item.operator === 'string' ? item.operator : undefined,
      expected: item.expected,
      actual: item.actual,
      message: typeof item.message === 'string' ? item.message : undefined,
    })
  }
  return rows.length > 0 ? rows : null
}

function rowsFromErrorDetail(output: unknown): AssertionRow[] | null {
  if (!isRecord(output)) {
    return null
  }
  const detail = output.detail
  if (!isRecord(detail)) {
    return null
  }
  return rowsFromResults(detail)
}

/** Assertions from assert node output or failed exec error detail. */
export function extractAssertionRows(execution: NodeExecutionDto): AssertionRow[] {
  const direct = rowsFromResults(execution.output)
  if (direct) {
    return direct
  }
  return rowsFromErrorDetail(execution.output) ?? []
}
