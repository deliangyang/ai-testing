import { useMemo, useState } from 'react'

import { filterWatchVariables, type WatchedVariable } from './watchVariables'

export interface VariableWatchProps {
  variables: WatchedVariable[]
}

export function VariableWatch({ variables }: VariableWatchProps) {
  const [textFilter, setTextFilter] = useState('')
  const filtered = useMemo(
    () => filterWatchVariables(variables, textFilter),
    [variables, textFilter],
  )

  return (
    <section className="variable-watch" data-testid="variable-watch">
      <div className="variable-watch__toolbar">
        <label className="run-log-panel__field run-log-panel__field--grow">
          过滤
          <input
            type="search"
            value={textFilter}
            placeholder="名称或值"
            onChange={(event) => {
              setTextFilter(event.target.value)
            }}
            data-testid="variable-watch-filter"
          />
        </label>
      </div>
      {filtered.length === 0 ? (
        <p className="run-log-list__empty">暂无变量</p>
      ) : (
        <table className="variable-watch__table">
          <thead>
            <tr>
              <th scope="col">变量</th>
              <th scope="col">值</th>
              <th scope="col">来源</th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((row) => (
              <tr key={row.name} data-testid={`variable-row-${row.name}`}>
                <td>{row.name}</td>
                <td>
                  <code>{row.value}</code>
                </td>
                <td>{row.source}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  )
}
