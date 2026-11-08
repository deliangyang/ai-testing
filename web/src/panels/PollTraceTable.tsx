import type { PollTraceTableRow } from './pollTrace'

export interface PollTraceTableProps {
  rows: PollTraceTableRow[]
}

export function PollTraceTable({ rows }: PollTraceTableProps) {
  return (
    <div className="poll-trace-table-wrap">
      <table className="poll-trace-table" data-testid="poll-trace-table">
        <thead>
          <tr>
            <th scope="col">节点</th>
            <th scope="col">第几次</th>
            <th scope="col">时间戳</th>
            <th scope="col">实际状态</th>
            <th scope="col">距期望值</th>
          </tr>
        </thead>
        <tbody>
          {rows.length === 0 ? (
            <tr>
              <td colSpan={5} className="poll-trace-table__empty">
                暂无轮询轨迹
              </td>
            </tr>
          ) : (
            rows.map((row) => (
              <tr
                key={`${row.nodeId}-${String(row.attempt)}`}
                data-testid={`poll-trace-row-${row.nodeId}-${String(row.attempt)}`}
                className={row.passed ? 'poll-trace-table__pass' : undefined}
              >
                <td>{row.nodeId}</td>
                <td>{row.attempt}</td>
                <td>{row.timestamp}</td>
                <td>{row.actual}</td>
                <td>{row.distance}</td>
              </tr>
            ))
          )}
        </tbody>
      </table>
    </div>
  )
}
