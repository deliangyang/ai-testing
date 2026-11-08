import type { NodeStatus } from '../data/types'

/** develop.md §5.2 execution view colors */
export const executionTokens = {
  pending: { border: '#6b7588', background: '#1e222c' },
  running: { border: '#5b8def', background: '#1a2438' },
  polling: { border: '#d4a72c', background: '#2a2414' },
  completed: { border: '#3d9e6a', background: '#15261c' },
  failed: { border: '#e25c5c', background: '#2a1818' },
  skipped: { border: '#4a4f5c', background: '#15171c' },
  progressTrack: '#2a2f3a',
  progressFill: '#5b8def',
} as const

type NodeChromeToken = { border: string; background: string }

export function statusPresentation(status: NodeStatus): {
  token: NodeChromeToken
  pulse: boolean
  cssClass: string
} {
  switch (status) {
    case 'PENDING':
    case 'READY':
      return { token: executionTokens.pending, pulse: false, cssClass: 'status-pending' }
    case 'RUNNING':
      return { token: executionTokens.running, pulse: true, cssClass: 'status-running' }
    case 'POLLING':
      return { token: executionTokens.polling, pulse: true, cssClass: 'status-polling' }
    case 'COMPLETED':
      return { token: executionTokens.completed, pulse: false, cssClass: 'status-completed' }
    case 'FAILED':
      return { token: executionTokens.failed, pulse: false, cssClass: 'status-failed' }
    case 'SKIPPED':
      return { token: executionTokens.skipped, pulse: false, cssClass: 'status-skipped' }
    default:
      return { token: executionTokens.pending, pulse: false, cssClass: 'status-pending' }
  }
}
