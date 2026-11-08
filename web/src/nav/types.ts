export type ScenarioRunStatus = 'COMPLETED' | 'FAILED' | 'CANCELLED' | 'RUNNING' | null

export type GateBadge = 'pass' | 'warn' | 'fail'

export interface ScenarioNavItem {
  id: string
  title: string
  lastRunStatus: ScenarioRunStatus
  /** 0–1 for the mini pass-rate bar */
  passRate: number
}

export interface PlanNavItem {
  id: string
  title: string
  gateStatus: GateBadge
}

export interface ReportNavItem {
  id: string
  title: string
  runStatus: string
  finishedAt: string | null
}

export interface NavCatalog {
  scenarios: ScenarioNavItem[]
  plans: PlanNavItem[]
  reports: ReportNavItem[]
}
