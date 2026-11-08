export interface RunReplayActionsProps {
  selectedNodeId: string | null
  sourceRunId?: string
  onRerunFromNode?: (nodeId: string) => void
}

export function RunReplayActions({
  selectedNodeId,
  sourceRunId,
  onRerunFromNode,
}: RunReplayActionsProps) {
  const disabled = !selectedNodeId || !onRerunFromNode
  return (
    <div className="dag-run-replay-actions" data-testid="run-replay-actions">
      <span className="dag-run-replay-actions__label">报告回放</span>
      {sourceRunId ? (
        <span className="dag-run-replay-actions__run" data-testid="replay-source-run">
          {sourceRunId}
        </span>
      ) : null}
      <button
        type="button"
        className="dag-run-replay-actions__btn"
        data-testid="replay-rerun-from-node"
        disabled={disabled}
        onClick={() => {
          if (selectedNodeId && onRerunFromNode) {
            onRerunFromNode(selectedNodeId)
          }
        }}
      >
        重跑此节点及下游
      </button>
    </div>
  )
}
