import { fireEvent, render, screen } from '@testing-library/react'
import { afterEach } from 'vitest'

import { AppShell } from './AppShell'
import { saveShellLayout } from './layoutStorage'
import { DEFAULT_SHELL_LAYOUT, workspaceScopeKey } from './shellLayout'
import { shellTokens } from './tokens'

describe('AppShell', () => {
  afterEach(() => {
    localStorage.clear()
  })

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

  it('restores layout from localStorage for the active workspace scope', () => {
    const key = workspaceScopeKey('scenario', 'async-poll')
    saveShellLayout(key, {
      ...DEFAULT_SHELL_LAYOUT,
      right_width_px: 280,
      nav_collapsed: true,
    })
    render(<AppShell />)
    expect(screen.getByTestId('app-shell')).toHaveAttribute('data-layout-scope', key)
    expect(screen.getByTestId('right-panel')).toHaveStyle({ width: '280px' })
    expect(screen.getByTestId('left-nav')).toHaveStyle({
      width: `${String(shellTokens.leftNavCollapsedWidthPx)}px`,
    })
  })

  it('applies a workspace preset to shell chrome', () => {
    render(<AppShell />)
    fireEvent.change(screen.getByTestId('workspace-preset-select'), {
      target: { value: 'reporter' },
    })
    expect(screen.getByTestId('left-nav')).toHaveStyle({
      width: `${String(shellTokens.leftNavCollapsedWidthPx)}px`,
    })
    expect(screen.getByTestId('right-panel')).toHaveStyle({ width: '300px' })
    expect(screen.getByTestId('bottom-drawer')).toHaveStyle({
      height: `${String(shellTokens.bottomDrawerCollapsedHeightPx)}px`,
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
