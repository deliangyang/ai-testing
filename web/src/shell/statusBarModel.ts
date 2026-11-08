import type { Dag } from '../types/generated'
import type { RunDto, StoredRunEvent } from '../data/types'
import { applyRunEvents, type RunViewModel } from '../canvas/runReplay'
import { STATUS_BAR_SHORTCUT_HINT } from './shortcutReference'

export interface ConnectionStatus {
  sqliteConnected: boolean
  sseConnected: boolean
}

export interface StatusBarModel {
  storageLabel: string
  storageConnected: boolean
  executionSummary: string
  sseLabel: string
  sseConnected: boolean
  envName: string
  elapsedLabel: string
  shortcutHint: string
}

export function countPollingNodes(view: RunViewModel): number {
  let count = 0
  for (const state of Object.values(view.nodeStates)) {
    if (state.status === 'POLLING') {
      count += 1
    }
  }
  return count
}

export function formatExecutionSummary(
  progress: { done: number; total: number },
  parallel: number,
  pollingCount: number,
): string {
  const parts = [`${String(progress.done)}/${String(progress.total)} 节点`, `并行 ${String(parallel)}`]
  if (pollingCount > 0) {
    parts.push(`${String(pollingCount)} 轮询中`)
  }
  return parts.join(' · ')
}

export function formatElapsed(run: RunDto): string {
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

export function buildStatusBarModel(input: {
  dag: Dag
  events: StoredRunEvent[]
  run: RunDto
  connections: ConnectionStatus
  parallel?: number
  replaySeq?: number
}): StatusBarModel {
  const parallel = input.parallel ?? 1
  const view = applyRunEvents(input.dag, input.events, input.replaySeq)
  const pollingCount = countPollingNodes(view)
  const executionSummary = formatExecutionSummary(view.progress, parallel, pollingCount)

  return {
    storageLabel: input.connections.sqliteConnected ? 'SQLite 已连接' : 'SQLite 断开',
    storageConnected: input.connections.sqliteConnected,
    executionSummary,
    sseLabel: input.connections.sseConnected ? 'SSE 已连接' : 'SSE 断开',
    sseConnected: input.connections.sseConnected,
    envName: input.run.env_name,
    elapsedLabel: `耗时 ${formatElapsed(input.run)}`,
    shortcutHint: STATUS_BAR_SHORTCUT_HINT,
  }
}
