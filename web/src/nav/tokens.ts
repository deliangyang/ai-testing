import type { GateBadge, ScenarioRunStatus } from './types'

export const navTokens = {
  passRateTrack: '#2a2f3a',
  passRateFill: '#3d9e6a',
  passRateFillLow: '#d4a72c',
  passRateFillFail: '#e25c5c',
} as const

export function runStatusDotClass(status: ScenarioRunStatus): string {
  switch (status) {
    case 'COMPLETED':
      return 'nav-status-dot--completed'
    case 'FAILED':
      return 'nav-status-dot--failed'
    case 'CANCELLED':
      return 'nav-status-dot--cancelled'
    case 'RUNNING':
      return 'nav-status-dot--running'
    default:
      return 'nav-status-dot--unknown'
  }
}

export function passRateFillColor(rate: number): string {
  if (rate >= 0.9) {
    return navTokens.passRateFill
  }
  if (rate >= 0.7) {
    return navTokens.passRateFillLow
  }
  return navTokens.passRateFillFail
}

export function gateBadgeLabel(status: GateBadge): string {
  switch (status) {
    case 'pass':
      return '✓'
    case 'warn':
      return '⚠'
    case 'fail':
      return '✗'
  }
}

export function gateBadgeClass(status: GateBadge): string {
  return `nav-gate-badge--${status}`
}

export function reportRunDotClass(runStatus: string): string {
  switch (runStatus) {
    case 'COMPLETED':
      return runStatusDotClass('COMPLETED')
    case 'FAILED':
      return runStatusDotClass('FAILED')
    case 'CANCELLED':
      return runStatusDotClass('CANCELLED')
    case 'RUNNING':
      return runStatusDotClass('RUNNING')
    default:
      return runStatusDotClass(null)
  }
}
