import type { GateBadge } from './types'
import { gateBadgeClass, gateBadgeLabel } from './tokens'

export interface ValidationBadgeProps {
  status: GateBadge
}

export function ValidationBadge({ status }: ValidationBadgeProps) {
  return (
    <span
      className={`nav-gate-badge ${gateBadgeClass(status)}`}
      data-testid={`gate-badge-${status}`}
      aria-label={`校验 ${status}`}
    >
      {gateBadgeLabel(status)}
    </span>
  )
}
