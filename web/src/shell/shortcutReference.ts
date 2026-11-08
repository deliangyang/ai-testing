export interface ShortcutRow {
  keys: string
  description: string
}

/** develop.md §5.3 item 5 */
export const SHORTCUT_REFERENCE: readonly ShortcutRow[] = [
  { keys: 'Ctrl/Cmd + K', description: '全局搜索场景与报告' },
  { keys: 'Ctrl/Cmd + Enter', description: '运行当前场景' },
  { keys: 'Ctrl/Cmd + Shift + Enter', description: '重跑失败节点' },
  { keys: 'F', description: '适应画布' },
  { keys: '1 / 2 / 3', description: '切换运行模式（单次 / 循环 / 指定节点）' },
  { keys: '?', description: '打开本快捷键面板' },
] as const

export const STATUS_BAR_SHORTCUT_HINT = 'Ctrl+K 搜索 · ? 快捷键'
