import { WORKSPACE_PRESETS } from './workspacePresets'

export interface WorkspacePresetSelectProps {
  onSelect: (presetId: string) => void
}

export function WorkspacePresetSelect({ onSelect }: WorkspacePresetSelectProps) {
  return (
    <label className="top-bar__preset">
      <span className="top-bar__preset-label">工作区</span>
      <select
        data-testid="workspace-preset-select"
        defaultValue=""
        onChange={(event) => {
          const id = event.target.value
          if (!id) {
            return
          }
          onSelect(id)
          event.target.value = ''
        }}
      >
        <option value="" disabled>
          选择预设…
        </option>
        {WORKSPACE_PRESETS.map((preset) => (
          <option key={preset.id} value={preset.id}>
            {preset.name}
          </option>
        ))}
      </select>
    </label>
  )
}
