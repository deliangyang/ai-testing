import { executionTokens } from './executionTokens'
import type { RunProgress } from './runReplay'

export interface RunProgressBarProps {
  progress: RunProgress
  label: string
}

export function RunProgressBar({ progress, label }: RunProgressBarProps) {
  const ratio = progress.total > 0 ? progress.done / progress.total : 0
  const percent = Math.round(ratio * 100)
  return (
    <div className="run-progress" data-testid="run-progress">
      <div className="run-progress__meta">
        <span data-testid="run-progress-label">{label}</span>
        <span>{percent}%</span>
      </div>
      <div
        className="run-progress__track"
        role="progressbar"
        aria-valuenow={progress.done}
        aria-valuemin={0}
        aria-valuemax={progress.total}
        aria-label={label}
        style={{ background: executionTokens.progressTrack }}
      >
        <div
          className="run-progress__fill"
          style={{
            width: `${String(percent)}%`,
            background: executionTokens.progressFill,
          }}
        />
      </div>
    </div>
  )
}
