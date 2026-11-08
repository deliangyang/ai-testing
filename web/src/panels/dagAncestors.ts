import type { Dag } from '../types/generated'

export function ancestorIds(dag: Dag, nodeId: string): string[] {
  const edges = dag.edges ?? []
  const preds = new Map<string, Set<string>>()
  for (const edge of edges) {
    const set = preds.get(edge.to) ?? new Set<string>()
    set.add(edge.from)
    preds.set(edge.to, set)
  }
  const visited = new Set<string>()
  const stack = [...(preds.get(nodeId) ?? [])]
  while (stack.length > 0) {
    const id = stack.pop()
    if (!id || visited.has(id)) {
      continue
    }
    visited.add(id)
    for (const p of preds.get(id) ?? []) {
      stack.push(p)
    }
  }
  return [...visited].sort()
}

export function upstreamVariableSuggestions(dag: Dag, nodeId: string): string[] {
  return ancestorIds(dag, nodeId).flatMap((id) => [
    `\${${id}.output.body}`,
    `\${${id}.output.status}`,
    `\${${id}.output.headers}`,
  ])
}
