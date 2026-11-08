import { shellTokens } from './tokens'

/** Persisted chrome layout for a workspace scope (mirrors store `ui_layouts.layout_json`). */
export interface ShellLayout {
  nav_collapsed: boolean
  right_width_px: number
  drawer_height_px: number
  drawer_collapsed: boolean
}

export const DEFAULT_SHELL_LAYOUT: ShellLayout = {
  nav_collapsed: false,
  right_width_px: shellTokens.rightPanelWidthPx,
  drawer_height_px: shellTokens.bottomDrawerHeightPx,
  drawer_collapsed: false,
}

export function mergeShellLayout(partial: Partial<ShellLayout> | null | undefined): ShellLayout {
  if (!partial) {
    return { ...DEFAULT_SHELL_LAYOUT }
  }
  return {
    nav_collapsed: partial.nav_collapsed ?? DEFAULT_SHELL_LAYOUT.nav_collapsed,
    right_width_px: partial.right_width_px ?? DEFAULT_SHELL_LAYOUT.right_width_px,
    drawer_height_px: partial.drawer_height_px ?? DEFAULT_SHELL_LAYOUT.drawer_height_px,
    drawer_collapsed: partial.drawer_collapsed ?? DEFAULT_SHELL_LAYOUT.drawer_collapsed,
  }
}

export function workspaceScopeKey(kind: string, refId: string): string {
  return `${kind}:${refId}`
}
