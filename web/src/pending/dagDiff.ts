import type { Dag } from '../types/generated'

export type DagDiffKind = 'added' | 'removed' | 'changed'

export interface DagDiffEntry {
  kind: DagDiffKind
  node_id: string
  detail: string
}

function nodeSummary(node: Dag['nodes'][number]): string {
  if (node.type === 'http') {
    return `${node.method} ${node.path}`
  }
  return node.type
}

/** 与 active 对比候选版本的节点级差异（确定性排序）。 */
export function diffDags(active: Dag, candidate: Dag): DagDiffEntry[] {
  const activeMap = new Map(active.nodes.map((n) => [n.id, n]))
  const candMap = new Map(candidate.nodes.map((n) => [n.id, n]))
  const ids = [...new Set([...activeMap.keys(), ...candMap.keys()])].sort()
  const rows: DagDiffEntry[] = []

  for (const id of ids) {
    const a = activeMap.get(id)
    const c = candMap.get(id)
    if (a === undefined && c !== undefined) {
      rows.push({ kind: 'added', node_id: id, detail: nodeSummary(c) })
    } else if (a !== undefined && c === undefined) {
      rows.push({ kind: 'removed', node_id: id, detail: nodeSummary(a) })
    } else if (a !== undefined && c !== undefined) {
      const left = nodeSummary(a)
      const right = nodeSummary(c)
      if (left !== right || a.type !== c.type) {
        rows.push({ kind: 'changed', node_id: id, detail: `${left} → ${right}` })
      }
    }
  }

  const edgeKey = (e: { from: string; to: string }) => `${e.from}->${e.to}`
  const activeEdges = new Set((active.edges ?? []).map(edgeKey))
  const candEdges = new Set((candidate.edges ?? []).map(edgeKey))
  for (const key of [...candEdges].filter((k) => !activeEdges.has(k)).sort()) {
    rows.push({ kind: 'added', node_id: key, detail: `边 ${key}` })
  }
  for (const key of [...activeEdges].filter((k) => !candEdges.has(k)).sort()) {
    rows.push({ kind: 'removed', node_id: key, detail: `边 ${key}` })
  }

  return rows
}
