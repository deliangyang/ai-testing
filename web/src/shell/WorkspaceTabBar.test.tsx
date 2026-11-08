import { fireEvent, render, screen } from '@testing-library/react'

import { WorkspaceTabBar } from './WorkspaceTabBar'
import { scenarioTabKey, reportTabKey, type WorkspaceTab } from './workspaceTabs'

describe('WorkspaceTabBar', () => {
  const tabs: WorkspaceTab[] = [
    {
      key: scenarioTabKey('serial'),
      kind: 'scenario',
      refId: 'serial',
      label: '场景 serial',
    },
    {
      key: reportTabKey('run-1'),
      kind: 'report',
      refId: 'run-1',
      label: '报告 run-1',
    },
  ]

  it('selects and closes tabs', () => {
    const onSelect = vi.fn()
    const onClose = vi.fn()
    render(
      <WorkspaceTabBar
        tabs={tabs}
        activeKey={tabs[0]?.key ?? null}
        onSelect={onSelect}
        onClose={onClose}
      />,
    )
    fireEvent.click(screen.getByRole('tab', { name: '报告 run-1' }))
    expect(onSelect).toHaveBeenCalledWith(reportTabKey('run-1'))
    fireEvent.click(screen.getByTestId(`workspace-tab-close-${reportTabKey('run-1')}`))
    expect(onClose).toHaveBeenCalledWith(reportTabKey('run-1'))
  })
})
