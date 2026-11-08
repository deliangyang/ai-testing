import { EnvironmentSelect } from './EnvironmentSelect'
import { RunControlBar } from './RunControlBar'
import type { RunControlState, RunMode } from './runControl'
import { WorkspacePresetSelect } from './WorkspacePresetSelect'

export interface TopBarProps {
  runControl: RunControlState
  onEnvChange: (env: string) => void
  onRun: () => void
  onStop: () => void
  onRerunFailed: () => void
  onModeChange: (mode: RunMode) => void
  onWorkspacePreset: (presetId: string) => void
}

export function TopBar(props: TopBarProps) {
  return (
    <div className="top-bar" data-testid="top-bar">
      <strong className="top-bar__brand">AITest</strong>
      <EnvironmentSelect value={props.runControl.envName} onChange={props.onEnvChange} />
      <RunControlBar
        state={props.runControl}
        onRun={props.onRun}
        onStop={props.onStop}
        onRerunFailed={props.onRerunFailed}
        onModeChange={props.onModeChange}
      />
      <span className="top-bar__spacer" />
      <WorkspacePresetSelect onSelect={props.onWorkspacePreset} />
      <span className="top-bar__meta">搜索 · 设置</span>
    </div>
  )
}
