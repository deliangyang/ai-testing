import { useState } from 'react'

import type { RunLogEntry } from '../focus/failureFocus'
import type { PollTraceTableRow } from '../panels/pollTrace'
import { PollTraceTable } from '../panels/PollTraceTable'
import { RunLogPanel } from './RunLogPanel'
import { VariableWatch } from './VariableWatch'
import type { WatchedVariable } from './watchVariables'

export type BottomDrawerTab = 'logs' | 'poll' | 'variables'

export interface BottomDrawerProps {
  logs: RunLogEntry[]
  pollRows: PollTraceTableRow[]
  variables: WatchedVariable[]
  initialNodeFilter?: string | null
  initialTab?: BottomDrawerTab
}

const TABS: { id: BottomDrawerTab; label: string }[] = [
  { id: 'logs', label: '日志' },
  { id: 'poll', label: '轮询' },
  { id: 'variables', label: '变量' },
]

export function BottomDrawer({
  logs,
  pollRows,
  variables,
  initialNodeFilter,
  initialTab = 'logs',
}: BottomDrawerProps) {
  const [tab, setTab] = useState<BottomDrawerTab>(initialTab)

  return (
    <div className="bottom-drawer" data-testid="bottom-drawer-panel">
      <div className="bottom-drawer__tabs" role="tablist" aria-label="底部抽屉">
        {TABS.map((item) => (
          <button
            key={item.id}
            type="button"
            role="tab"
            aria-selected={tab === item.id}
            className={`bottom-drawer__tab${tab === item.id ? ' bottom-drawer__tab--active' : ''}`}
            onClick={() => {
              setTab(item.id)
            }}
            data-testid={`drawer-tab-${item.id}`}
          >
            {item.label}
          </button>
        ))}
      </div>
      <div className="bottom-drawer__content" role="tabpanel">
        {tab === 'logs' ? (
          <RunLogPanel entries={logs} initialNodeFilter={initialNodeFilter} />
        ) : null}
        {tab === 'poll' ? <PollTraceTable rows={pollRows} /> : null}
        {tab === 'variables' ? <VariableWatch variables={variables} /> : null}
      </div>
    </div>
  )
}
