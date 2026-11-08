import { useMemo } from 'react'
import {
  Background,
  Controls,
  MiniMap,
  Panel,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  type NodeTypes,
} from '@xyflow/react'

import type { Dag } from '../types/generated'
import type { StoredRunEvent } from '../data/types'
import { buildDagRunGraph, DAG_RUN_NODE_TYPE } from './buildDagRunGraph'
import { DagRunNodeCard } from './DagRunNodeCard'
import { RunProgressBar } from './RunProgressBar'
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

function DagRunCanvasInner({
  dag,
  events,
  replaySeq,
}: {
  dag: Dag
  events: StoredRunEvent[]
  replaySeq?: number
}) {
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
      fitView
      fitViewOptions={{ padding: 0.2 }}
      nodesDraggable={false}
      nodesConnectable={false}
      elementsSelectable
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
      <Panel position="top-left">
        <RunProgressBar progress={view.progress} label={label} />
      </Panel>
      <Panel position="top-right">
        <FitViewControl />
      </Panel>
    </ReactFlow>
  )
}

export interface DagRunCanvasProps {
  dag: Dag
  events: StoredRunEvent[]
  replaySeq?: number
}

export function DagRunCanvas({ dag, events, replaySeq }: DagRunCanvasProps) {
  return (
    <div className="dag-edit-canvas" data-testid="dag-run-canvas" aria-label="DAG 执行态画布">
      <ReactFlowProvider>
        <DagRunCanvasInner dag={dag} events={events} replaySeq={replaySeq} />
      </ReactFlowProvider>
    </div>
  )
}
