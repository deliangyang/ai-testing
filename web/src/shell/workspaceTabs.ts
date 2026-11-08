export type WorkspaceTabKind = 'scenario' | 'report'

export interface WorkspaceTab {
  key: string
  kind: WorkspaceTabKind
  refId: string
  label: string
}

export function scenarioTabKey(scenarioId: string): string {
  return `scenario:${scenarioId}`
}

export function reportTabKey(reportId: string): string {
  return `report:${reportId}`
}

export function openWorkspaceTab(tabs: WorkspaceTab[], tab: WorkspaceTab): WorkspaceTab[] {
  if (tabs.some((item) => item.key === tab.key)) {
    return tabs
  }
  return [...tabs, tab].sort((a, b) => a.key.localeCompare(b.key))
}

export function closeWorkspaceTab(tabs: WorkspaceTab[], key: string): WorkspaceTab[] {
  return tabs.filter((tab) => tab.key !== key)
}

export function activeTabAfterClose(
  tabs: WorkspaceTab[],
  closedKey: string,
  previousActive: string,
): string | null {
  if (tabs.length === 0) {
    return null
  }
  if (previousActive !== closedKey) {
    return tabs.some((tab) => tab.key === previousActive) ? previousActive : (tabs[0]?.key ?? null)
  }
  return tabs[0]?.key ?? null
}

export function tabLabelForScenario(title: string): string {
  return `场景 ${title}`
}

export function tabLabelForReport(title: string): string {
  return `报告 ${title}`
}
