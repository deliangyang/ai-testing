import type { NodeExecutionDto } from '../data/types'

export interface TimingBreakdown {
  wallMs: number | null
  requestMs: number | null
}

function parseMs(iso: string | null | undefined): number | null {
  if (!iso) {
    return null
  }
  const t = Date.parse(iso)
  return Number.isNaN(t) ? null : t
}

function requestDurationMs(output: unknown): number | null {
  if (output === null || typeof output !== 'object') {
    return null
  }
  const timing = (output as { timing?: unknown }).timing
  if (timing === null || typeof timing !== 'object') {
    return null
  }
  const ms = (timing as { duration_ms?: unknown }).duration_ms
  return typeof ms === 'number' && Number.isFinite(ms) ? ms : null
}

export function timingBreakdown(execution: NodeExecutionDto): TimingBreakdown {
  const start = parseMs(execution.started_at)
  const end = parseMs(execution.finished_at)
  const wallMs = start !== null && end !== null && end >= start ? end - start : null
  return {
    wallMs,
    requestMs: requestDurationMs(execution.output),
  }
}

export function formatTimingBreakdown(timing: TimingBreakdown): string[] {
  const lines: string[] = []
  if (timing.wallMs !== null) {
    lines.push(`节点总耗时 ${String(timing.wallMs)} ms`)
  }
  if (timing.requestMs !== null) {
    lines.push(`请求耗时 ${String(timing.requestMs)} ms`)
  }
  return lines
}
