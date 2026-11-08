import { describe, expect, it } from 'vitest'

import { gateBadgeFromReport } from './gateStatus'

describe('gateBadgeFromReport', () => {
  it('maps report to pass warn fail', () => {
    expect(gateBadgeFromReport({ passed: true, issues: [] })).toBe('pass')
    expect(
      gateBadgeFromReport({
        passed: true,
        issues: [{ code: 'x', severity: 'warn', message: 'm' }],
      }),
    ).toBe('warn')
    expect(
      gateBadgeFromReport({
        passed: false,
        issues: [{ code: 'x', severity: 'error', message: 'm' }],
      }),
    ).toBe('fail')
  })
})
