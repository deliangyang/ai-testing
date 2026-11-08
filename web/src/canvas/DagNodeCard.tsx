import { Handle, Position, type NodeProps, type Node as FlowNode } from '@xyflow/react'

import type { DagNodeData } from './dagToFlow'
import { canvasTokens } from './tokens'

export type DagFlowNode = FlowNode<DagNodeData>

export function DagNodeCard({ data, selected }: NodeProps<DagFlowNode>) {
  return (
    <article
      className={`dag-node-card${selected ? ' dag-node-card--selected' : ''}`}
      data-testid={`dag-node-${data.dagNode.id}`}
      style={{
        borderColor: selected ? canvasTokens.nodeBorderSelected : canvasTokens.nodeBorder,
        background: canvasTokens.nodeBackground,
      }}
    >
      <Handle type="target" position={Position.Top} className="dag-node-card__handle" />
      <div className="dag-node-card__head">
        <span className="dag-node-card__icon" aria-hidden>
          {data.icon}
        </span>
        <span className="dag-node-card__title">{data.title}</span>
      </div>
      <p className="dag-node-card__summary" title={data.summary}>
        {data.summary}
      </p>
      <Handle type="source" position={Position.Bottom} className="dag-node-card__handle" />
    </article>
  )
}
