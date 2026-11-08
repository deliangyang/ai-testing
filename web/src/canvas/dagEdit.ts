import type { Dag, Edge, Node } from '../types/generated'

function sortedNodes(nodes: Node[]): Node[] {
  return [...nodes].sort((a, b) => a.id.localeCompare(b.id))
}

function sortedEdges(edges: Edge[] | undefined): Edge[] {
  if (!edges?.length) {
    return []
  }
  return [...edges].sort((a, b) => {
    const f = a.from.localeCompare(b.from)
    return f !== 0 ? f : a.to.localeCompare(b.to)
  })
}

export function nextHttpNodeId(dag: Dag, prefix = 'node'): string {
  const used = new Set(dag.nodes.map((n) => n.id))
  let index = 1
  let candidate = `${prefix}_${String(index)}`
  while (used.has(candidate)) {
    index += 1
    candidate = `${prefix}_${String(index)}`
  }
  return candidate
}

export function createHttpNode(id: string): Node {
  return { id, type: 'http', method: 'GET', path: '/' }
}

export function addHttpNode(dag: Dag, id?: string): Dag {
  const nodeId = id ?? nextHttpNodeId(dag)
  if (dag.nodes.some((n) => n.id === nodeId)) {
    throw new Error(`node '${nodeId}' already exists`)
  }
  return {
    ...dag,
    nodes: sortedNodes([...dag.nodes, createHttpNode(nodeId)]),
  }
}

export function removeNode(dag: Dag, nodeId: string): Dag {
  return {
    ...dag,
    nodes: sortedNodes(dag.nodes.filter((n) => n.id !== nodeId)),
    edges: sortedEdges(dag.edges?.filter((e) => e.from !== nodeId && e.to !== nodeId)),
  }
}

export function addEdge(dag: Dag, from: string, to: string): Dag {
  if (from === to) {
    return dag
  }
  const edges = dag.edges ?? []
  if (edges.some((e) => e.from === from && e.to === to)) {
    return dag
  }
  if (!dag.nodes.some((n) => n.id === from) || !dag.nodes.some((n) => n.id === to)) {
    return dag
  }
  return {
    ...dag,
    edges: sortedEdges([...edges, { from, to }]),
  }
}

export function removeEdge(dag: Dag, from: string, to: string): Dag {
  return {
    ...dag,
    edges: sortedEdges(dag.edges?.filter((e) => !(e.from === from && e.to === to))),
  }
}

export function edgeCount(dag: Dag): number {
  return dag.edges?.length ?? 0
}
