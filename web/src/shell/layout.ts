import { shellTokens } from './tokens'

export function clampRightPanelWidth(widthPx: number): number {
  return Math.min(
    shellTokens.rightPanelMaxWidthPx,
    Math.max(shellTokens.rightPanelMinWidthPx, Math.round(widthPx)),
  )
}

export function clampBottomDrawerHeight(heightPx: number): number {
  return Math.min(
    shellTokens.bottomDrawerMaxHeightPx,
    Math.max(shellTokens.bottomDrawerMinHeightPx, Math.round(heightPx)),
  )
}

export function leftNavWidthPx(collapsed: boolean): number {
  return collapsed ? shellTokens.leftNavCollapsedWidthPx : shellTokens.leftNavWidthPx
}
