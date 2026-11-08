import type { RunLogEntry } from '../focus/failureFocus'

export interface RunLogListProps {
  entries: RunLogEntry[]
  focusNodeId?: string | null
}

export function RunLogList({ entries, focusNodeId }: RunLogListProps) {
  return (
    <section
      className="run-log-list"
      data-testid="run-log-list"
      data-focus-node={focusNodeId ?? undefined}
      aria-label="运行日志"
    >
      <h3 className="run-log-list__title">
        实时日志
        {focusNodeId ? <span className="run-log-list__filter"> · 节点 {focusNodeId}</span> : null}
      </h3>
      {entries.length === 0 ? (
        <p className="run-log-list__empty">暂无日志</p>
      ) : (
        <ul className="run-log-list__items">
          {entries.map((entry) => (
            <li
              key={String(entry.seq)}
              className={`run-log-list__item run-log-list__item--${entry.level}`}
              data-testid={`run-log-${String(entry.seq)}`}
            >
              <span className="run-log-list__level">{entry.level}</span>
              {entry.node ? <span className="run-log-list__node">{entry.node}</span> : null}
              <span className="run-log-list__message">{entry.message}</span>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}
