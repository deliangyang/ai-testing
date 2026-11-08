import type { RunControlState, RunMode } from './runControl'

export interface RunControlBarProps {
  state: RunControlState
  onRun: () => void
  onStop: () => void
  onRerunFailed: () => void
  onModeChange: (mode: RunMode) => void
}

const MODE_OPTIONS: { value: RunMode; label: string }[] = [
  { value: 'once', label: '单次' },
  { value: 'loop', label: '循环' },
  { value: 'from_node', label: '指定节点' },
]

export function RunControlBar({
  state,
  onRun,
  onStop,
  onRerunFailed,
  onModeChange,
}: RunControlBarProps) {
  const running = state.phase === 'running' || state.phase === 'cancelling'
  return (
    <div className="top-bar__run" data-testid="run-control-bar">
      <span className="top-bar__run-phase" data-testid="run-phase">
        {state.phase}
      </span>
      <select
        className="top-bar__run-mode"
        aria-label="运行模式"
        data-testid="run-mode-select"
        value={state.mode}
        disabled={running}
        onChange={(event) => {
          onModeChange(event.target.value as RunMode)
        }}
      >
        {MODE_OPTIONS.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
      <button
        type="button"
        className="top-bar__run-btn"
        data-testid="run-control-run"
        disabled={running}
        onClick={onRun}
      >
        运行
      </button>
      <button
        type="button"
        className="top-bar__run-btn"
        data-testid="run-control-stop"
        disabled={!running}
        onClick={onStop}
      >
        停止
      </button>
      <button
        type="button"
        className="top-bar__run-btn"
        data-testid="run-control-rerun-failed"
        disabled={!state.canRerunFailed || running}
        onClick={onRerunFailed}
      >
        重跑失败
      </button>
    </div>
  )
}
