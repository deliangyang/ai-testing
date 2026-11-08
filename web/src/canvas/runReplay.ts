import type { Dag, Node } from '../types/generated'
import type { NodeStatus, RunTerminalStatus, StoredRunEvent } from '../data/types'

export interface NodeRunState {
  status: NodeStatus
  pollAttempt?: number
  pollMaxAttempts?: number | null
  pollActual?: string
}

export interface RunProgress {
  done: number
  total: number
}

export interface RunViewModel {
  nodeStates: Record<string, NodeRunState>
  runStatus?: RunTerminalStatus
  progress: RunProgress
}

function sortedNodeIds(dag: Dag): string[] {
  return [...new Set(dag.nodes.map((n) => n.id))].sort()
}

function pollMaxAttempts(node: Node): number | null {
  if (node.type !== 'poll') {
    return null
  }
  if (node.max_attempts != null) {
    return node.max_attempts
  }
  return null
}

function pollActualFromEntry(entry: unknown): string {
  if (entry !== null && typeof entry === 'object' && 'actual' in entry) {
    const raw = (entry as { actual?: unknown }).actual
    if (typeof raw === 'string') {
      return raw
    }
    if (raw === null || raw === undefined) {
      return '—'
    }
    return JSON.stringify(raw)
  }
  return '—'
}

function isTerminal(status: NodeStatus): boolean {
  return status === 'COMPLETED' || status === 'FAILED' || status === 'SKIPPED'
}

export function formatPollStatusLine(state: NodeRunState): string | undefined {
  if (state.status !== 'POLLING' && state.pollAttempt === undefined) {
    return undefined
  }
  if (state.pollAttempt === undefined) {
    return undefined
  }
  const max =
    state.pollMaxAttempts != null && state.pollMaxAttempts > 0
      ? `/${String(state.pollMaxAttempts)}`
      : ''
  const actual = state.pollActual ?? '—'
  return `第 ${String(state.pollAttempt)}${max} 次 · 状态=${actual}`
}

function initialStates(dag: Dag): Record<string, NodeRunState> {
  const pollMax = new Map(dag.nodes.map((n) => [n.id, pollMaxAttempts(n)]))
  const out: Record<string, NodeRunState> = {}
  for (const id of sortedNodeIds(dag)) {
    out[id] = {
      status: 'PENDING',
      pollMaxAttempts: pollMax.get(id) ?? null,
    }
  }
  return out
}

/**
 * Deterministic fold of stored SSE events into per-node execution visualization state.
 */
export function applyRunEvents(dag: Dag, events: StoredRunEvent[], upToSeq?: number): RunViewModel {
  const nodeStates = initialStates(dag)
  const sorted = [...events].sort((a, b) => a.seq - b.seq)
  const limit = upToSeq ?? Number.POSITIVE_INFINITY
  let runStatus: RunTerminalStatus | undefined

  for (const item of sorted) {
    if (item.seq > limit) {
      break
    }
    const event = item.event
    switch (event.type) {
      case 'node_state_changed': {
        nodeStates[event.node].status = event.status
        break
      }
      case 'poll_iteration': {
        const current = nodeStates[event.node]
        current.pollAttempt = event.attempt
        current.pollActual = pollActualFromEntry(event.entry)
        if (current.status !== 'POLLING') {
          current.status = 'POLLING'
        }
        break
      }
      case 'run_finished':
        runStatus = event.status
        break
      case 'log':
        break
    }
  }

  const total = sortedNodeIds(dag).length
  let done = 0
  for (const id of sortedNodeIds(dag)) {
    if (isTerminal(nodeStates[id].status)) {
      done += 1
    }
  }

  return { nodeStates, runStatus, progress: { done, total } }
}

export function runProgressLabel(progress: RunProgress, runStatus?: RunTerminalStatus): string {
  const base = `${String(progress.done)}/${String(progress.total)} 节点`
  if (runStatus === 'COMPLETED') {
    return `${base} · 已完成`
  }
  if (runStatus === 'FAILED') {
    return `${base} · 失败`
  }
  if (runStatus === 'CANCELLED') {
    return `${base} · 已取消`
  }
  if (progress.done < progress.total) {
    return `${base} · 执行中`
  }
  return base
}
