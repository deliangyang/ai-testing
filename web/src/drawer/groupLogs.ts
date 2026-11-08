import type { RunLogEntry } from '../focus/failureFocus'

export const GLOBAL_LOG_GROUP = '__global__'

export interface LogGroup {
  nodeId: string
  label: string
  entries: RunLogEntry[]
}

export function groupLogsByNode(entries: RunLogEntry[]): LogGroup[] {
  const buckets = new Map<string, RunLogEntry[]>()
  for (const entry of entries) {
    const key = entry.node ?? GLOBAL_LOG_GROUP
    const list = buckets.get(key)
    if (list) {
      list.push(entry)
    } else {
      buckets.set(key, [entry])
    }
  }
  return [...buckets.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([nodeId, groupEntries]) => ({
      nodeId,
      label: nodeId === GLOBAL_LOG_GROUP ? '全局' : nodeId,
      entries: [...groupEntries].sort((a, b) => a.seq - b.seq),
    }))
}

export function filterLogEntries(
  entries: RunLogEntry[],
  options: { nodeId?: string | null; text?: string },
): RunLogEntry[] {
  const nodeId = options.nodeId
  const text = options.text?.trim().toLowerCase() ?? ''
  return entries.filter((entry) => {
    if (nodeId && nodeId !== 'all') {
      if (entry.node !== nodeId) {
        return false
      }
    }
    if (text.length > 0) {
      const hay = `${entry.message} ${entry.node ?? ''} ${entry.level}`.toLowerCase()
      if (!hay.includes(text)) {
        return false
      }
    }
    return true
  })
}

export function logNodeIds(entries: RunLogEntry[]): string[] {
  const ids = new Set<string>()
  for (const entry of entries) {
    if (entry.node) {
      ids.add(entry.node)
    }
  }
  return [...ids].sort()
}
