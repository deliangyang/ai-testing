/** Layout defaults from develop.md §5.1 */
export const shellTokens = {
  topBarHeightPx: 48,
  statusBarHeightPx: 24,
  leftNavWidthPx: 240,
  leftNavCollapsedWidthPx: 48,
  rightPanelWidthPx: 360,
  rightPanelMinWidthPx: 240,
  rightPanelMaxWidthPx: 560,
  bottomDrawerHeightPx: 200,
  bottomDrawerMinHeightPx: 120,
  bottomDrawerMaxHeightPx: 480,
  bottomDrawerCollapsedHeightPx: 32,
  resizeHandleSizePx: 6,
} as const

export type ShellTokens = typeof shellTokens
