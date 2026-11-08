import dagre from '@dagrejs/dagre'

import type { Dag, Edge } from '../../types/generated'
import { layoutTokens } from './tokens'

export interface LayoutNodePosition {
  id: string
  /** Top-left in layout coordinates (pixels). */
  x: number
  y: number
  width: number
  height: number
}

export interface DagLayout {
  nodes: LayoutNodePosition[]
  width: number
  height: number
}

export interface LayoutOptions {
  rankdir?: 'TB' | 'BT' | 'LR' | 'RL'
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

function roundPx(value: number): number {
  return Math.round(value * 100) / 100
}

/**
 * Deterministic Dagre layout: same `dag` document yields identical coordinates
 * (sorted node list, stable edge order, fixed spacing).
 */
export function layoutDag(dag: Dag, options: LayoutOptions = {}): DagLayout {
  const g = new dagre.graphlib.Graph()
  g.setGraph({
    rankdir: options.rankdir ?? 'TB',
    nodesep: layoutTokens.nodesepPx,
    ranksep: layoutTokens.ranksepPx,
    marginx: layoutTokens.marginPx,
    marginy: layoutTokens.marginPx,
  })
  g.setDefaultEdgeLabel(() => ({}))

  for (const id of sortedNodeIds(dag)) {
    g.setNode(id, {
      width: layoutTokens.nodeWidthPx,
      height: layoutTokens.nodeHeightPx,
    })
  }

  for (const edge of sortedEdges(dag.edges)) {
    g.setEdge(edge.from, edge.to)
  }

  dagre.layout(g)

  const positions: LayoutNodePosition[] = sortedNodeIds(dag).map((id) => {
    const placed = g.node(id) as { x: number; y: number; width: number; height: number }
    return {
      id,
      x: roundPx(placed.x - placed.width / 2),
      y: roundPx(placed.y - placed.height / 2),
      width: layoutTokens.nodeWidthPx,
      height: layoutTokens.nodeHeightPx,
    }
  })

  const graph = g.graph() as { width?: number; height?: number }
  return {
    nodes: positions,
    width: roundPx(graph.width ?? layoutTokens.nodeWidthPx),
    height: roundPx(graph.height ?? layoutTokens.nodeHeightPx),
  }
}
