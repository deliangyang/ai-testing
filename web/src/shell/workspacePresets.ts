import type { ShellLayout } from './shellLayout'

export interface WorkspacePreset {
  id: 'orchestrator' | 'debugger' | 'reporter'
  name: string
  layout: ShellLayout
}

/** Mirrors `workspace_presets` seed rows in `V003__ui_layout.sql`. */
export const WORKSPACE_PRESETS: readonly WorkspacePreset[] = [
  {
    id: 'orchestrator',
    name: '编排者',
    layout: {
      nav_collapsed: false,
      right_width_px: 400,
      drawer_height_px: 140,
      drawer_collapsed: true,
    },
  },
  {
    id: 'debugger',
    name: '执行调试者',
    layout: {
      nav_collapsed: false,
      right_width_px: 360,
      drawer_height_px: 220,
      drawer_collapsed: false,
    },
  },
  {
    id: 'reporter',
    name: '看报告者',
    layout: {
      nav_collapsed: true,
      right_width_px: 300,
      drawer_height_px: 140,
      drawer_collapsed: true,
    },
  },
] as const

export function workspacePresetById(id: string): WorkspacePreset | undefined {
  return WORKSPACE_PRESETS.find((preset) => preset.id === id)
}
