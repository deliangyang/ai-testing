import { fireEvent, render, screen } from '@testing-library/react'

import { AppShell } from './AppShell'
import { scenarioTabKey } from './workspaceTabs'

describe('AppShell workspace tabs', () => {
  it('opens a scenario tab when double-clicking the nav item', () => {
    render(<AppShell />)
    expect(screen.getByTestId('workspace-tabs')).toBeInTheDocument()
    fireEvent.doubleClick(screen.getByTestId('nav-scenario-serial'))
    expect(screen.getByTestId(`workspace-tab-${scenarioTabKey('serial')}`)).toBeInTheDocument()
    fireEvent.click(screen.getByRole('tab', { name: '场景 serial' }))
    expect(screen.getByTestId('dag-run-canvas')).toBeInTheDocument()
  })
})
