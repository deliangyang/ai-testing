import { describe, expect, it } from 'vitest'

import { SERIAL_DAG, SERIAL_EVENTS, SERIAL_RUN } from '../data/fixtures/serial'
import { buildRunReportSummary, maxEventSeq } from './runReportSummary'

describe('buildRunReportSummary', () => {
  it('computes pass rate and mode for a completed serial run', () => {
    const summary = buildRunReportSummary(SERIAL_DAG, SERIAL_EVENTS, SERIAL_RUN)
    expect(summary.passRatePercent).toBe(100)
    expect(summary.modeLabel).toBe('手动')
    expect(summary.elapsed).toMatch(/ms|s/)
  })

  it('compares against a previous partial run', () => {
    const previous = {
      run: {
        ...SERIAL_RUN,
        started_at: '2026-11-08T13:59:00.000+08:00',
        finished_at: '2026-11-08T13:59:03.500+08:00',
      },
      events: SERIAL_EVENTS.filter((e) => e.seq <= 5),
    }
    const summary = buildRunReportSummary(SERIAL_DAG, SERIAL_EVENTS, SERIAL_RUN, previous)
    expect(summary.vsLastPassRate).toBe('+33%')
    expect(summary.vsLastElapsed).toMatch(/^-/)
  })
})

describe('maxEventSeq', () => {
  it('returns the highest seq', () => {
    expect(maxEventSeq(SERIAL_EVENTS)).toBe(7)
  })
})
