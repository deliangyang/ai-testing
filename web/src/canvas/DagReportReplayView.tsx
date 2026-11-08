import { useMemo, useState } from 'react'

import type { RunDto, StoredRunEvent } from '../data/types'
import type { Dag } from '../types/generated'
import { DagRunCanvas } from './DagRunCanvas'
import { RunReplayTimeline } from './RunReplayTimeline'
import { RunReportSummaryBar } from './RunReportSummaryBar'
import { buildRunReportSummary, maxEventSeq } from './runReportSummary'

export interface DagReportReplayViewProps {
  dag: Dag
  events: StoredRunEvent[]
  run: RunDto
  previous?: { run: RunDto; events: StoredRunEvent[] }
  focusNodeId?: string | null
  sourceRunId?: string
  partialRerunNode?: string | null
  onRerunFromNode?: (nodeId: string) => void
  fitViewToken?: number
}

export function DagReportReplayView({
  dag,
  events,
  run,
  previous,
  focusNodeId,
  sourceRunId,
  partialRerunNode,
  onRerunFromNode,
  fitViewToken,
}: DagReportReplayViewProps) {
  const maxSeq = maxEventSeq(events)
  const [replaySeq, setReplaySeq] = useState(maxSeq)
  const summary = useMemo(
    () => buildRunReportSummary(dag, events, run, previous),
    [dag, events, run, previous],
  )
  const effectiveSeq = replaySeq >= maxSeq ? undefined : replaySeq

  return (
    <div className="dag-report-replay" data-testid="dag-report-replay">
      <RunReportSummaryBar summary={summary} />
      <RunReplayTimeline maxSeq={maxSeq} value={replaySeq} onChange={setReplaySeq} />
      <div className="dag-report-replay__canvas">
        <DagRunCanvas
          dag={dag}
          events={events}
          replaySeq={effectiveSeq}
          replayMode
          focusNodeId={focusNodeId}
          sourceRunId={sourceRunId ?? run.id}
          partialRerunNode={partialRerunNode}
          onRerunFromNode={onRerunFromNode}
          fitViewToken={fitViewToken}
        />
      </div>
    </div>
  )
}
