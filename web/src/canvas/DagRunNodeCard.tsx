import { Handle, Position, type NodeProps, type Node as FlowNode } from '@xyflow/react'

import type { DagRunNodeData } from './buildDagRunGraph'
import { statusPresentation } from './executionTokens'
import { canvasTokens } from './tokens'

export type DagRunFlowNode = FlowNode<DagRunNodeData>

export function DagRunNodeCard({ data, selected }: NodeProps<DagRunFlowNode>) {
  const { token, pulse, cssClass } = statusPresentation(data.runState.status)
  return (
    <article
      className={`dag-node-card dag-node-card--run ${cssClass}${pulse ? ' dag-node-card--pulse' : ''}${selected ? ' dag-node-card--selected' : ''}`}
      data-testid={`dag-run-node-${data.dagNode.id}`}
      data-status={data.runState.status}
      style={{
        borderColor: selected ? canvasTokens.nodeBorderSelected : token.border,
        background: token.background,
      }}
    >
      <Handle type="target" position={Position.Top} className="dag-node-card__handle" />
      <div className="dag-node-card__head">
        <span className="dag-node-card__icon" aria-hidden>
          {data.icon}
        </span>
        <span className="dag-node-card__title">{data.title}</span>
      </div>
      {data.pollLine ? (
        <p className="dag-node-card__poll" data-testid={`dag-poll-line-${data.dagNode.id}`}>
          {data.pollLine}
        </p>
      ) : (
        <p className="dag-node-card__summary" title={data.summary}>
          {data.summary}
        </p>
      )}
      <Handle type="source" position={Position.Bottom} className="dag-node-card__handle" />
    </article>
  )
}
