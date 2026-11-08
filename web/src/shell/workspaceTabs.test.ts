import {
  activeTabAfterClose,
  openWorkspaceTab,
  reportTabKey,
  scenarioTabKey,
  type WorkspaceTab,
} from './workspaceTabs'

describe('workspaceTabs', () => {
  const serial: WorkspaceTab = {
    key: scenarioTabKey('serial'),
    kind: 'scenario',
    refId: 'serial',
    label: '场景 serial',
  }
  const report: WorkspaceTab = {
    key: reportTabKey('run-1'),
    kind: 'report',
    refId: 'run-1',
    label: '报告 run-1',
  }

  it('opens tabs without duplicates', () => {
    let tabs = openWorkspaceTab([], serial)
    expect(tabs).toHaveLength(1)
    tabs = openWorkspaceTab(tabs, serial)
    expect(tabs).toHaveLength(1)
    tabs = openWorkspaceTab(tabs, report)
    expect(tabs.map((t) => t.key)).toEqual([report.key, serial.key])
  })

  it('picks a sensible tab after close', () => {
    const tabs = [serial, report]
    expect(activeTabAfterClose(tabs, report.key, report.key)).toBe(serial.key)
    expect(activeTabAfterClose(tabs, serial.key, report.key)).toBe(report.key)
  })
})
