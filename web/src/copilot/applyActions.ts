import { addEdge } from '../canvas/dagEdit'
import type { CopilotAction } from '../data/types'
import type { Dag, Node } from '../types/generated'

function sortedNodes(nodes: Node[]): Node[] {
  return [...nodes].sort((a, b) => a.id.localeCompare(b.id))
}

export interface CopilotActionResult {
  dag: Dag
  focusNodeId?: string
  selectedNodeId?: string
}

export function applyCopilotAction(dag: Dag, action: CopilotAction): CopilotActionResult {
  switch (action.type) {
    case 'insert_canvas': {
      let next = dag
      if (!dag.nodes.some((n) => n.id === action.node.id)) {
        next = {
          ...dag,
          nodes: sortedNodes([...dag.nodes, action.node]),
        }
      }
      for (const edge of action.edges ?? []) {
        next = addEdge(next, edge.from, edge.to)
      }
      return {
        dag: next,
        focusNodeId: action.node.id,
        selectedNodeId: action.node.id,
      }
    }
    case 'focus_node':
      return {
        dag,
        focusNodeId: action.node_id,
        selectedNodeId: action.node_id,
      }
    case 'adopt_assertion': {
      const nodes = dag.nodes.map((node) => {
        if (node.id !== action.node_id || node.type !== 'assert') {
          return node
        }
        const merged = [...node.assertions]
        for (const assertion of action.assertions) {
          const dup = merged.some((m) => m.path === assertion.path && m.op === assertion.op)
          if (!dup) {
            merged.unshift(assertion)
          }
        }
        return { ...node, assertions: merged }
      })
      return {
        dag: { ...dag, nodes: sortedNodes(nodes) },
        selectedNodeId: action.node_id,
      }
    }
    default: {
      const _exhaustive: never = action
      return { dag: _exhaustive }
    }
  }
}
