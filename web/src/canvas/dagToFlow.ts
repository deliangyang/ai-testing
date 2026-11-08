import { MarkerType, type Edge as FlowEdge, type Node as FlowNode } from '@xyflow/react'

import type { Dag, Edge } from '../types/generated'
import { layoutDag, type LayoutOptions } from './layout'
import { edgeVariableLabel, nodeDisplayName, nodeSummary, nodeTypeIcon } from './nodeMeta'

export const DAG_NODE_TYPE = 'dagNode' as const

export interface DagNodeData {
  dagNode: import('../types/generated').Node
  title: string
  summary: string
  icon: string
  [key: string]: unknown
}

function sortedNodeIds(dag: Dag): string[] {
  return [...new Set(dag.nodes.map((n) => n.id))].sort()
}

function sortedEdges(edges: Edge[] | undefined): Edge[] {
  if (!edges) {
    return []
  }
  return [...edges].sort((a, b) => {
    const f = a.from.localeCompare(b.from)
    if (f !== 0) {
      return f
    }
    return a.to.localeCompare(b.to)
  })
}

export function buildDagEditGraph(
  dag: Dag,
  layoutOptions: LayoutOptions = {},
): { nodes: FlowNode<DagNodeData>[]; edges: FlowEdge[] } {
  const layout = layoutDag(dag, layoutOptions)
  const byId = new Map(dag.nodes.map((n) => [n.id, n]))
  const posById = new Map(layout.nodes.map((p) => [p.id, p]))

  const nodes: FlowNode<DagNodeData>[] = sortedNodeIds(dag).map((id) => {
    const dagNode = byId.get(id)
    const placed = posById.get(id)
    if (!dagNode || !placed) {
      throw new Error(`layout missing node ${id}`)
    }
    return {
      id,
      type: DAG_NODE_TYPE,
      position: { x: placed.x, y: placed.y },
      data: {
        dagNode,
        title: nodeDisplayName(dagNode),
        summary: nodeSummary(dagNode),
        icon: nodeTypeIcon(dagNode.type),
      },
      width: placed.width,
      height: placed.height,
    }
  })

  const edges: FlowEdge[] = sortedEdges(dag.edges).map((edge) => {
    const toNode = byId.get(edge.to)
    const label = toNode !== undefined ? edgeVariableLabel(edge.from, toNode) : undefined
    return {
      id: `${edge.from}->${edge.to}`,
      source: edge.from,
      target: edge.to,
      type: 'default',
      markerEnd: { type: MarkerType.ArrowClosed, width: 18, height: 18 },
      label,
      labelStyle: label ? { fill: '#c4a35a', fontSize: 11 } : undefined,
      labelBgStyle: label ? { fill: '#1e222c', fillOpacity: 0.9 } : undefined,
    }
  })

  return { nodes, edges }
}
