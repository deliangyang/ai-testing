import type { PollTraceEntry } from './pollTrace'
import { pollDistanceLabel } from './pollTrace'

export interface PollTraceTimelineProps {
  nodeId: string
  entries: PollTraceEntry[]
}

function formatActual(value: unknown): string {
  if (value === undefined || value === null) {
    return '—'
  }
  return typeof value === 'string' ? value : JSON.stringify(value)
}

export function PollTraceTimeline({ nodeId, entries }: PollTraceTimelineProps) {
  if (entries.length === 0) {
    return null
  }
  return (
    <section className="panel-section" data-testid="poll-trace-timeline">
      <h3 className="panel-section__title">轮询轨迹 · {nodeId}</h3>
      <ol className="poll-timeline">
        {entries.map((entry) => (
          <li
            key={`${nodeId}-${String(entry.attempt)}`}
            className={`poll-timeline__item${entry.passed ? ' poll-timeline__item--passed' : ''}`}
            data-testid={`poll-timeline-${nodeId}-${String(entry.attempt)}`}
          >
            <div className="poll-timeline__attempt">第 {entry.attempt} 次</div>
            <div className="poll-timeline__meta">
              <span>{entry.ts ?? '—'}</span>
              <span>实际 {formatActual(entry.actual)}</span>
            </div>
            <div className="poll-timeline__distance">{pollDistanceLabel(entry)}</div>
          </li>
        ))}
      </ol>
    </section>
  )
}
