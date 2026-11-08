import { useCallback, useMemo, useState, type MouseEvent as DomMouseEvent } from 'react'
import {
  Background,
  Controls,
  MiniMap,
  Panel,
  ReactFlow,
  ReactFlowProvider,
  useOnSelectionChange,
  useReactFlow,
  type Node,
  type NodeTypes,
} from '@xyflow/react'

import type { Dag } from '../types/generated'
import type { StoredRunEvent } from '../data/types'
import { buildDagRunGraph, DAG_RUN_NODE_TYPE } from './buildDagRunGraph'
import { DagRunNodeCard } from './DagRunNodeCard'
import { FocusNodeEffect } from './focusNodeEffect'
import { RunNodeContextMenu } from './RunNodeContextMenu'
import { RunProgressBar } from './RunProgressBar'
import { RunReplayActions } from './RunReplayActions'
import { applyRunEvents, runProgressLabel } from './runReplay'
import { canvasTokens } from './tokens'
import './canvas.css'

import '@xyflow/react/dist/style.css'

const nodeTypes: NodeTypes = {
  [DAG_RUN_NODE_TYPE]: DagRunNodeCard,
}

function FitViewControl() {
  const { fitView } = useReactFlow()
  return (
    <button
      type="button"
      className="dag-edit-canvas__fit"
      onClick={() => {
        void fitView({ padding: 0.2, duration: 200 })
      }}
    >
      适应画布
    </button>
  )
}

function SelectionBridge({
  onChange,
}: {
  onChange: (nodeId: string | null) => void
}) {
  useOnSelectionChange({
    onChange: ({ nodes }) => {
      onChange(nodes[0]?.id ?? null)
    },
  })
  return null
}

function DagRunCanvasInner({
  dag,
  events,
  replaySeq,
  focusNodeId,
  replayMode,
  sourceRunId,
  onRerunFromNode,
}: {
  dag: Dag
  events: StoredRunEvent[]
  replaySeq?: number
  focusNodeId?: string | null
  replayMode?: boolean
  sourceRunId?: string
  onRerunFromNode?: (nodeId: string) => void
}) {
  const [contextMenu, setContextMenu] = useState<{ nodeId: string; x: number; y: number } | null>(
    null,
  )
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null)

  const handleRerun = useCallback(
    (nodeId: string) => {
      onRerunFromNode?.(nodeId)
    },
    [onRerunFromNode],
  )

  const onNodeContextMenu = useCallback(
    (event: DomMouseEvent, node: Node) => {
      if (!onRerunFromNode) {
        return
      }
      event.preventDefault()
      setContextMenu({ nodeId: node.id, x: event.clientX, y: event.clientY })
    },
    [onRerunFromNode],
  )

  const onNodeClick = useCallback((_: DomMouseEvent, node: Node) => {
    setSelectedNodeId(node.id)
  }, [])
  const view = useMemo(
    () => applyRunEvents(dag, events, replaySeq),
    [dag, events, replaySeq],
  )
  const { nodes, edges } = useMemo(
    () => buildDagRunGraph(dag, view.nodeStates),
    [dag, view.nodeStates],
  )
  const label = runProgressLabel(view.progress, view.runStatus)

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      nodeTypes={nodeTypes}
      fitView={!focusNodeId}
      fitViewOptions={{ padding: 0.2 }}
      nodesDraggable={false}
      nodesConnectable={false}
      elementsSelectable
      onNodeContextMenu={onNodeContextMenu}
      onNodeClick={onNodeClick}
      proOptions={{ hideAttribution: true }}
      color={canvasTokens.edgeStroke}
    >
      <SelectionBridge onChange={setSelectedNodeId} />
      <Background gap={16} color="#2a2f3a" />
      <Controls showInteractive={false} />
      <MiniMap
        pannable
        zoomable
        maskColor={canvasTokens.minimapMask}
        nodeColor={() => canvasTokens.nodeBackground}
      />
      <Panel position="top-left">
        <RunProgressBar progress={view.progress} label={label} />
      </Panel>
      <Panel position="top-right">
        <FitViewControl />
      </Panel>
      {replayMode ? (
        <Panel position="bottom-center">
          <RunReplayActions
            selectedNodeId={selectedNodeId}
            sourceRunId={sourceRunId}
            onRerunFromNode={onRerunFromNode}
          />
        </Panel>
      ) : null}
      <FocusNodeEffect nodeId={focusNodeId} />
      {contextMenu ? (
        <RunNodeContextMenu
          nodeId={contextMenu.nodeId}
          x={contextMenu.x}
          y={contextMenu.y}
          onRerun={handleRerun}
          onClose={() => {
            setContextMenu(null)
          }}
        />
      ) : null}
    </ReactFlow>
  )
}

export interface DagRunCanvasProps {
  dag: Dag
  events: StoredRunEvent[]
  replaySeq?: number
  focusNodeId?: string | null
  /** 报告回放：展示「重跑此节点及下游」条 */
  replayMode?: boolean
  /** 部分重跑所依据的已存档 run id */
  sourceRunId?: string
  onRerunFromNode?: (nodeId: string) => void
  /** 最近一次触发的部分重跑节点（供测试与状态展示） */
  partialRerunNode?: string | null
}

export function DagRunCanvas({
  dag,
  events,
  replaySeq,
  focusNodeId,
  replayMode,
  sourceRunId,
  onRerunFromNode,
  partialRerunNode,
}: DagRunCanvasProps) {
  return (
    <div
      className="dag-edit-canvas"
      data-testid="dag-run-canvas"
      data-focus-node={focusNodeId ?? undefined}
      data-partial-rerun-node={partialRerunNode ?? undefined}
      data-partial-rerun-from-run={partialRerunNode ? sourceRunId : undefined}
      aria-label="DAG 执行态画布"
    >
      <ReactFlowProvider>
        <DagRunCanvasInner
          dag={dag}
          events={events}
          replaySeq={replaySeq}
          focusNodeId={focusNodeId}
          replayMode={replayMode}
          sourceRunId={sourceRunId}
          onRerunFromNode={onRerunFromNode}
        />
      </ReactFlowProvider>
    </div>
  )
}
