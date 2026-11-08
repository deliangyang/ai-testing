import type { GateResponse } from '../data/types'
import type { GateBadge } from '../nav/types'

export function gateBadgeFromReport(report: GateResponse): GateBadge {
  if (!report.passed) {
    return 'fail'
  }
  if (report.issues.some((issue) => issue.severity === 'warn')) {
    return 'warn'
  }
  return 'pass'
}
