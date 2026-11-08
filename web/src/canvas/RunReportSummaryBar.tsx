import type { RunReportSummary } from './runReportSummary'

export interface RunReportSummaryBarProps {
  summary: RunReportSummary
}

export function RunReportSummaryBar({ summary }: RunReportSummaryBarProps) {
  return (
    <div className="run-report-summary" data-testid="run-report-summary">
      <span data-testid="run-report-pass-rate">
        通过率 {summary.passRatePercent}%（{summary.passed}/{summary.total}）
      </span>
      <span data-testid="run-report-elapsed">耗时 {summary.elapsed}</span>
      <span data-testid="run-report-mode">触发 {summary.modeLabel}</span>
      <span data-testid="run-report-vs-pass">
        较上次 {summary.vsLastPassRate ?? '—'}
      </span>
      <span data-testid="run-report-vs-elapsed">
        耗时差 {summary.vsLastElapsed ?? '—'}
      </span>
    </div>
  )
}
