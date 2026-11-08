import type { Dag, Node } from '../types/generated'

export function findNode(dag: Dag, nodeId: string): Node | undefined {
  return dag.nodes.find((n) => n.id === nodeId)
}

export function replaceNode(dag: Dag, node: Node): Dag {
  return {
    ...dag,
    nodes: dag.nodes.map((n) => (n.id === node.id ? node : n)),
  }
}
