import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  Background,
  Controls,
  MiniMap,
  Panel,
  ReactFlow,
  ReactFlowProvider,
  useEdgesState,
  useNodesState,
  useReactFlow,
  type Connection,
  type NodeTypes,
  type OnConnect,
  type OnEdgesDelete,
} from '@xyflow/react'

import type { Dag } from '../types/generated'
import { DagEditContextMenu, type DagEditMenuAction } from './DagEditContextMenu'
import { DagNodeCard } from './DagNodeCard'
import { addEdge as dagAddEdge, addHttpNode, removeEdge, removeNode } from './dagEdit'
import { buildDagEditGraph, DAG_NODE_TYPE } from './dagToFlow'
import { FocusNodeEffect } from './focusNodeEffect'
import { canvasTokens } from './tokens'
import './canvas.css'

import '@xyflow/react/dist/style.css'

const nodeTypes: NodeTypes = {
  [DAG_NODE_TYPE]: DagNodeCard,
}

function FitViewControl() {
  const { fitView } = useReactFlow()
  return (
    <button
      type="button"
      className="dag-edit-canvas__fit"
      data-testid="dag-edit-fit-view"
      onClick={() => {
        void fitView({ padding: 0.2, duration: 200 })
      }}
    >
      适应画布
    </button>
  )
}

function AutoLayoutControl({ onLayout }: { onLayout: () => void }) {
  return (
    <button
      type="button"
      className="dag-edit-canvas__fit"
      data-testid="dag-edit-auto-layout"
      onClick={onLayout}
    >
      自动布局
    </button>
  )
}

function DagEditCanvasInner({
  dag,
  onDagChange,
  onSelectedNodeIdChange,
  focusNodeId,
}: {
  dag: Dag
  onDagChange: (dag: Dag) => void
  onSelectedNodeIdChange?: (nodeId: string | null) => void
  focusNodeId?: string | null
}) {
  const { fitView } = useReactFlow()
  const [layoutPass, setLayoutPass] = useState(0)
  const [menu, setMenu] = useState<
    { target: 'pane' | 'node'; nodeId?: string; x: number; y: number } | null
  >(null)

  const graph = useMemo(() => buildDagEditGraph(dag), [dag, layoutPass])
  const [nodes, setNodes, onNodesChange] = useNodesState(graph.nodes)
  const [edges, setEdges, onEdgesChange] = useEdgesState(graph.edges)

  useEffect(() => {
    setNodes(graph.nodes)
    setEdges(graph.edges)
  }, [graph, setNodes, setEdges])

  const commitDag = useCallback(
    (next: Dag) => {
      onDagChange(next)
    },
    [onDagChange],
  )

  const onConnect: OnConnect = useCallback(
    (connection: Connection) => {
      if (!connection.source || !connection.target) {
        return
      }
      commitDag(dagAddEdge(dag, connection.source, connection.target))
    },
    [commitDag, dag],
  )

  const onEdgesDelete: OnEdgesDelete = useCallback(
    (removed) => {
      let next = dag
      for (const edge of removed) {
        next = removeEdge(next, edge.source, edge.target)
      }
      commitDag(next)
    },
    [commitDag, dag],
  )

  const openPaneMenu = (clientX: number, clientY: number) => {
    setMenu({ target: 'pane', x: clientX, y: clientY })
  }

  const openNodeMenu = (nodeId: string, clientX: number, clientY: number) => {
    setMenu({ target: 'node', nodeId, x: clientX, y: clientY })
  }

  const onMenuAction = useCallback(
    (action: DagEditMenuAction) => {
      if (action.kind === 'add-http-node') {
        commitDag(addHttpNode(dag))
        return
      }
      commitDag(removeNode(dag, action.nodeId))
    },
    [commitDag, dag],
  )

  const runAutoLayout = useCallback(() => {
    setLayoutPass((n) => n + 1)
    queueMicrotask(() => {
      void fitView({ padding: 0.2, duration: 200 })
    })
  }, [fitView])

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      nodeTypes={nodeTypes}
      onNodesChange={onNodesChange}
      onEdgesChange={onEdgesChange}
      onConnect={onConnect}
      onEdgesDelete={onEdgesDelete}
      onPaneContextMenu={(event) => {
        event.preventDefault()
        openPaneMenu(event.clientX, event.clientY)
      }}
      onNodeContextMenu={(event, node) => {
        event.preventDefault()
        openNodeMenu(node.id, event.clientX, event.clientY)
      }}
      onNodeClick={(_, node) => {
        onSelectedNodeIdChange?.(node.id)
      }}
      onPaneClick={() => {
        onSelectedNodeIdChange?.(null)
      }}
      fitView={layoutPass === 0}
      fitViewOptions={{ padding: 0.2 }}
      nodesDraggable
      nodesConnectable
      elementsSelectable
      deleteKeyCode={['Backspace', 'Delete']}
      proOptions={{ hideAttribution: true }}
      color={canvasTokens.edgeStroke}
    >
      <Background gap={16} color="#2a2f3a" />
      <Controls showInteractive={false} />
      <MiniMap
        pannable
        zoomable
        maskColor={canvasTokens.minimapMask}
        nodeColor={() => canvasTokens.nodeBackground}
      />
      <Panel position="top-right">
        <div className="dag-edit-canvas__toolbar">
          <AutoLayoutControl onLayout={runAutoLayout} />
          <FitViewControl />
        </div>
      </Panel>
      <FocusNodeEffect nodeId={focusNodeId} />
      {menu ? (
        <DagEditContextMenu
          x={menu.x}
          y={menu.y}
          target={menu.target}
          nodeId={menu.nodeId}
          onAction={onMenuAction}
          onClose={() => {
            setMenu(null)
          }}
        />
      ) : null}
    </ReactFlow>
  )
}

export interface DagEditCanvasProps {
  dag: Dag
  onDagChange?: (dag: Dag) => void
  onSelectedNodeIdChange?: (nodeId: string | null) => void
  focusNodeId?: string | null
}

function DagEditCanvasControlled(props: DagEditCanvasProps) {
  const { dag, onDagChange } = props
  const [draft, setDraft] = useState(dag)
  useEffect(() => {
    setDraft(dag)
  }, [dag])

  const change = onDagChange ?? setDraft

  return (
    <div
      className="dag-edit-canvas"
      data-testid="dag-edit-canvas"
      data-node-count={draft.nodes.length}
      data-edge-count={draft.edges?.length ?? 0}
      data-focus-node={props.focusNodeId ?? undefined}
      aria-label="DAG 编辑画布"
    >
      <ReactFlowProvider>
        <DagEditCanvasInner
          dag={draft}
          onDagChange={change}
          onSelectedNodeIdChange={props.onSelectedNodeIdChange}
          focusNodeId={props.focusNodeId}
        />
      </ReactFlowProvider>
    </div>
  )
}

export function DagEditCanvas(props: DagEditCanvasProps) {
  return <DagEditCanvasControlled {...props} />
}
