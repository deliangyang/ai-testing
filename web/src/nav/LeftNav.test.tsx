import { render, screen, within } from '@testing-library/react'

import { FIXTURE_NAV_CATALOG } from './navCatalog'
import { LeftNav } from './LeftNav'
import './nav.css'

describe('LeftNav', () => {
  it('lists scenarios with status dots and pass-rate bars', () => {
    render(<LeftNav catalog={FIXTURE_NAV_CATALOG} collapsed={false} onToggleCollapse={() => {}} />)
    const serial = screen.getByTestId('nav-scenario-serial')
    expect(within(serial).getByTestId('nav-status-serial')).toHaveClass('nav-status-dot--completed')
    expect(within(serial).getByTestId('pass-rate-bar')).toHaveAttribute('aria-valuenow', '100')
    const asyncItem = screen.getByTestId('nav-scenario-async-poll')
    expect(within(asyncItem).getByTestId('nav-status-async-poll')).toHaveClass('nav-status-dot--failed')
  })

  it('shows plan validation badges', () => {
    render(<LeftNav catalog={FIXTURE_NAV_CATALOG} collapsed={false} onToggleCollapse={() => {}} />)
    expect(screen.getByTestId('gate-badge-pass')).toHaveTextContent('✓')
    expect(screen.getByTestId('gate-badge-warn')).toHaveTextContent('⚠')
    expect(screen.getByTestId('gate-badge-fail')).toHaveTextContent('✗')
  })

  it('lists reports from fixture runs', () => {
    render(<LeftNav catalog={FIXTURE_NAV_CATALOG} collapsed={false} onToggleCollapse={() => {}} />)
    expect(screen.getByTestId('nav-report-run-fixture-serial')).toHaveTextContent('serial')
    expect(screen.getByTestId('nav-report-run-async-poll-fail')).toHaveTextContent('async-poll')
  })

  it('hides pass-rate bars when collapsed', () => {
    render(<LeftNav catalog={FIXTURE_NAV_CATALOG} collapsed onToggleCollapse={() => {}} />)
    expect(screen.queryByTestId('pass-rate-bar')).not.toBeInTheDocument()
  })
})
