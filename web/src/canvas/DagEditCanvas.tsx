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
import { DagNodeCard } from './DagNodeCard'
import { buildDagEditGraph, DAG_NODE_TYPE } from './dagToFlow'
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
      onClick={() => {
        void fitView({ padding: 0.2, duration: 200 })
      }}
    >
      适应画布
    </button>
  )
}

function DagEditCanvasInner({ dag }: { dag: Dag }) {
  const { nodes, edges } = useMemo(() => buildDagEditGraph(dag), [dag])

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
      <Panel position="top-right">
        <FitViewControl />
      </Panel>
    </ReactFlow>
  )
}

export interface DagEditCanvasProps {
  dag: Dag
}

export function DagEditCanvas({ dag }: DagEditCanvasProps) {
  return (
    <div className="dag-edit-canvas" data-testid="dag-edit-canvas" aria-label="DAG 编辑画布">
      <ReactFlowProvider>
        <DagEditCanvasInner dag={dag} />
      </ReactFlowProvider>
    </div>
  )
}
