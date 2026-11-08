import { useMemo, useState } from 'react'

import type { RunLogEntry } from '../focus/failureFocus'
import { filterLogEntries, groupLogsByNode, logNodeIds } from './groupLogs'

export interface RunLogPanelProps {
  entries: RunLogEntry[]
  initialNodeFilter?: string | null
}

export function RunLogPanel({ entries, initialNodeFilter }: RunLogPanelProps) {
  const [nodeFilter, setNodeFilter] = useState<string>(() => initialNodeFilter ?? 'all')
  const [textFilter, setTextFilter] = useState('')

  const nodeIds = useMemo(() => logNodeIds(entries), [entries])
  const filtered = useMemo(
    () =>
      filterLogEntries(entries, {
        nodeId: nodeFilter === 'all' ? null : nodeFilter,
        text: textFilter,
      }),
    [entries, nodeFilter, textFilter],
  )
  const groups = useMemo(() => groupLogsByNode(filtered), [filtered])

  return (
    <section className="run-log-panel" data-testid="run-log-panel">
      <div className="run-log-panel__toolbar">
        <label className="run-log-panel__field">
          节点
          <select
            value={nodeFilter}
            onChange={(event) => {
              setNodeFilter(event.target.value)
            }}
            data-testid="run-log-node-filter"
          >
            <option value="all">全部</option>
            {nodeIds.map((id) => (
              <option key={id} value={id}>
                {id}
              </option>
            ))}
          </select>
        </label>
        <label className="run-log-panel__field run-log-panel__field--grow">
          过滤
          <input
            type="search"
            value={textFilter}
            placeholder="消息关键字"
            onChange={(event) => {
              setTextFilter(event.target.value)
            }}
            data-testid="run-log-text-filter"
          />
        </label>
      </div>
      {groups.length === 0 ? (
        <p className="run-log-list__empty">暂无匹配的日志</p>
      ) : (
        <div className="run-log-panel__groups">
          {groups.map((group) => (
            <section
              key={group.nodeId}
              className="run-log-panel__group"
              data-testid={`run-log-group-${group.nodeId}`}
            >
              <h4 className="run-log-panel__group-title">{group.label}</h4>
              <ul className="run-log-list__items">
                {group.entries.map((entry) => (
                  <li
                    key={String(entry.seq)}
                    className={`run-log-list__item run-log-list__item--${entry.level}`}
                    data-testid={`run-log-${String(entry.seq)}`}
                  >
                    <span className="run-log-list__level">{entry.level}</span>
                    <span className="run-log-list__message">{entry.message}</span>
                  </li>
                ))}
              </ul>
            </section>
          ))}
        </div>
      )}
    </section>
  )
}
