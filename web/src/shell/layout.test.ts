import { clampBottomDrawerHeight, clampRightPanelWidth, leftNavWidthPx } from './layout'
import { shellTokens } from './tokens'

describe('shell layout clamps', () => {
  it('clamps right panel to 240–560px', () => {
    expect(clampRightPanelWidth(0)).toBe(shellTokens.rightPanelMinWidthPx)
    expect(clampRightPanelWidth(239)).toBe(shellTokens.rightPanelMinWidthPx)
    expect(clampRightPanelWidth(360)).toBe(360)
    expect(clampRightPanelWidth(561)).toBe(shellTokens.rightPanelMaxWidthPx)
    expect(clampRightPanelWidth(900)).toBe(shellTokens.rightPanelMaxWidthPx)
  })

  it('clamps bottom drawer height', () => {
    expect(clampBottomDrawerHeight(50)).toBe(shellTokens.bottomDrawerMinHeightPx)
    expect(clampBottomDrawerHeight(200)).toBe(200)
    expect(clampBottomDrawerHeight(600)).toBe(shellTokens.bottomDrawerMaxHeightPx)
  })

  it('left nav toggles between expanded and icon rail', () => {
    expect(leftNavWidthPx(false)).toBe(shellTokens.leftNavWidthPx)
    expect(leftNavWidthPx(true)).toBe(shellTokens.leftNavCollapsedWidthPx)
  })
})
