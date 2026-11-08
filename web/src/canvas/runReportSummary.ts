import type { Dag } from '../types/generated'
import type { RunDto, StoredRunEvent } from '../data/types'
import { applyRunEvents } from './runReplay'

export interface RunReportSummary {
  passed: number
  total: number
  passRatePercent: number
  elapsed: string
  modeLabel: string
  vsLastPassRate: string | null
  vsLastElapsed: string | null
}

function elapsedLabel(run: RunDto): string {
  const start = run.started_at ? Date.parse(run.started_at) : Number.NaN
  const end = run.finished_at ? Date.parse(run.finished_at) : Number.NaN
  if (Number.isNaN(start) || Number.isNaN(end) || end < start) {
    return '—'
  }
  const ms = end - start
  if (ms < 1000) {
    return `${String(ms)} ms`
  }
  return `${(ms / 1000).toFixed(1)}s`
}

function elapsedMs(run: RunDto): number | null {
  const start = run.started_at ? Date.parse(run.started_at) : Number.NaN
  const end = run.finished_at ? Date.parse(run.finished_at) : Number.NaN
  if (Number.isNaN(start) || Number.isNaN(end) || end < start) {
    return null
  }
  return end - start
}

function passStats(dag: Dag, events: StoredRunEvent[]): { passed: number; total: number } {
  const view = applyRunEvents(dag, events)
  let passed = 0
  for (const state of Object.values(view.nodeStates)) {
    if (state.status === 'COMPLETED') {
      passed += 1
    }
  }
  return { passed, total: view.progress.total }
}

function modeLabel(mode: string): string {
  if (mode === 'MANUAL') {
    return '手动'
  }
  if (mode === 'CI') {
    return 'CI'
  }
  return mode
}

function formatDeltaMs(delta: number): string {
  const sign = delta >= 0 ? '+' : ''
  if (Math.abs(delta) < 1000) {
    return `${sign}${String(delta)} ms`
  }
  return `${sign}${(delta / 1000).toFixed(1)}s`
}

export function buildRunReportSummary(
  dag: Dag,
  events: StoredRunEvent[],
  run: RunDto,
  previous?: { run: RunDto; events: StoredRunEvent[] },
): RunReportSummary {
  const { passed, total } = passStats(dag, events)
  const passRatePercent = total > 0 ? Math.round((passed / total) * 100) : 0

  let vsLastPassRate: string | null = null
  let vsLastElapsed: string | null = null
  if (previous) {
    const prev = passStats(dag, previous.events)
    const prevRate = prev.total > 0 ? Math.round((prev.passed / prev.total) * 100) : 0
    const deltaRate = passRatePercent - prevRate
    vsLastPassRate = deltaRate === 0 ? '0%' : `${deltaRate > 0 ? '+' : ''}${String(deltaRate)}%`
    const ms = elapsedMs(run)
    const prevMs = elapsedMs(previous.run)
    if (ms !== null && prevMs !== null) {
      vsLastElapsed = formatDeltaMs(ms - prevMs)
    }
  }

  return {
    passed,
    total,
    passRatePercent,
    elapsed: elapsedLabel(run),
    modeLabel: modeLabel(run.mode),
    vsLastPassRate,
    vsLastElapsed,
  }
}

export function maxEventSeq(events: StoredRunEvent[]): number {
  return events.reduce((max, item) => Math.max(max, item.seq), 0)
}
