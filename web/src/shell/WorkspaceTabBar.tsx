import type { WorkspaceTab } from './workspaceTabs'

export interface WorkspaceTabBarProps {
  tabs: WorkspaceTab[]
  activeKey: string | null
  onSelect: (key: string) => void
  onClose: (key: string) => void
}

export function WorkspaceTabBar({ tabs, activeKey, onSelect, onClose }: WorkspaceTabBarProps) {
  return (
    <div className="app-shell__tabs" role="tablist" aria-label="工作区标签" data-testid="workspace-tabs">
      {tabs.map((tab) => {
        const selected = tab.key === activeKey
        return (
          <span key={tab.key} className="workspace-tab" data-testid={`workspace-tab-${tab.key}`}>
            <button
              type="button"
              role="tab"
              aria-selected={selected}
              className={`workspace-tab__select${selected ? ' workspace-tab__select--active' : ''}`}
              onClick={() => {
                onSelect(tab.key)
              }}
            >
              {tab.label}
            </button>
            {tabs.length > 1 ? (
              <button
                type="button"
                className="workspace-tab__close"
                aria-label={`关闭 ${tab.label}`}
                onClick={() => {
                  onClose(tab.key)
                }}
                data-testid={`workspace-tab-close-${tab.key}`}
              >
                ×
              </button>
            ) : null}
          </span>
        )
      })}
    </div>
  )
}
