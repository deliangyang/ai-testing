import type { Edge as FlowEdge, Node as FlowNode } from '@xyflow/react'

import type { Dag } from '../types/generated'
import { buildDagEditGraph, type DagNodeData } from './dagToFlow'
import { formatPollStatusLine, type NodeRunState } from './runReplay'

export const DAG_RUN_NODE_TYPE = 'dagRunNode' as const

export interface DagRunNodeData extends DagNodeData {
  runState: NodeRunState
  pollLine?: string
}

export function buildDagRunGraph(
  dag: Dag,
  nodeStates: Record<string, NodeRunState>,
): { nodes: FlowNode<DagRunNodeData>[]; edges: FlowEdge[] } {
  const base = buildDagEditGraph(dag)
  const nodes: FlowNode<DagRunNodeData>[] = base.nodes.map((node) => {
    const runState = nodeStates[node.id] ?? { status: 'PENDING' as const }
    return {
      ...node,
      type: DAG_RUN_NODE_TYPE,
      data: {
        ...node.data,
        runState,
        pollLine: formatPollStatusLine(runState),
      },
    }
  })
  return { nodes, edges: base.edges }
}
