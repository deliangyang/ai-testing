import type { StatusBarModel } from './statusBarModel'

export interface StatusBarProps {
  model: StatusBarModel
}

export function StatusBar({ model }: StatusBarProps) {
  return (
    <footer className="app-shell__status" role="contentinfo" data-testid="status-bar">
      <span
        data-testid="status-storage"
        className={model.storageConnected ? 'status-bar__ok' : 'status-bar__bad'}
      >
        ● {model.storageLabel}
      </span>
      <span data-testid="status-execution">{model.executionSummary}</span>
      <span
        data-testid="status-sse"
        className={model.sseConnected ? 'status-bar__ok' : 'status-bar__bad'}
      >
        {model.sseLabel}
      </span>
      <span data-testid="status-env">环境 {model.envName}</span>
      <span data-testid="status-elapsed">{model.elapsedLabel}</span>
    </footer>
  )
}
