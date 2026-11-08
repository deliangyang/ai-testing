import { fireEvent, render, screen } from '@testing-library/react'

import { AppShell } from './AppShell'
import { shellTokens } from './tokens'

describe('AppShell', () => {
  it('uses develop.md default region sizes', () => {
    render(<AppShell />)
    const left = screen.getByTestId('left-nav')
    const right = screen.getByTestId('right-panel')
    const drawer = screen.getByTestId('bottom-drawer')
    const status = screen.getByTestId('status-bar')

    expect(left).toHaveStyle({
      width: `${String(shellTokens.leftNavWidthPx)}px`,
    })
    expect(right).toHaveStyle({
      width: `${String(shellTokens.rightPanelWidthPx)}px`,
    })
    expect(drawer).toHaveStyle({
      height: `${String(shellTokens.bottomDrawerHeightPx)}px`,
    })
    expect(status).toHaveClass('app-shell__status')
  })

  it('collapses left navigation to the icon rail width', () => {
    render(<AppShell />)
    fireEvent.click(screen.getByRole('button', { name: '折叠左导航' }))
    expect(screen.getByTestId('left-nav')).toHaveStyle({
      width: `${String(shellTokens.leftNavCollapsedWidthPx)}px`,
    })
  })

  it('collapses the bottom drawer to the toolbar height', () => {
    render(<AppShell />)
    fireEvent.click(screen.getByRole('button', { name: '折叠底部抽屉' }))
    expect(screen.getByTestId('bottom-drawer')).toHaveStyle({
      height: `${String(shellTokens.bottomDrawerCollapsedHeightPx)}px`,
    })
  })
})
