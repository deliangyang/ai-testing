import type { GateResponse } from '../data/types'
import { ValidationBadge } from '../nav/ValidationBadge'
import { gateBadgeFromReport } from './gateStatus'
import './panels.css'

export interface DagValidationPanelProps {
  report: GateResponse | null
  onFocusNode?: (nodeId: string) => void
}

export function DagValidationPanel({ report, onFocusNode }: DagValidationPanelProps) {
  if (!report) {
    return (
      <div className="dag-validation dag-validation--loading" data-testid="dag-validation-panel">
        校验中…
      </div>
    )
  }
  const badge = gateBadgeFromReport(report)
  return (
    <div className="dag-validation" data-testid="dag-validation-panel" data-gate={badge}>
      <div className="dag-validation__head">
        <span>闸门校验</span>
        <ValidationBadge status={badge} />
      </div>
      {report.issues.length === 0 ? (
        <p className="dag-validation__ok">无问题</p>
      ) : (
        <ul className="dag-validation__issues" data-testid="dag-validation-issues">
          {report.issues.map((issue) => (
            <li key={`${issue.code}-${issue.node_id ?? ''}-${issue.message}`}>
              <button
                type="button"
                className="dag-validation__issue"
                data-testid={issue.node_id ? `gate-issue-${issue.node_id}` : 'gate-issue-global'}
                onClick={() => {
                  if (issue.node_id) {
                    onFocusNode?.(issue.node_id)
                  }
                }}
              >
                <span className={`dag-validation__sev dag-validation__sev--${issue.severity}`}>
                  {issue.severity === 'error' ? '✗' : '⚠'}
                </span>
                <span>{issue.message}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
