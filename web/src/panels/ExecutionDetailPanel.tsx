import type { NodeExecutionDto } from '../data/types'
import { extractAssertionRows, type AssertionRow } from './assertionResults'
import { formatDisplayValue } from './formatValue'
import { formatTimingBreakdown, timingBreakdown } from './executionTiming'
import { parsePollTrace } from './pollTrace'
import { PollTraceTimeline } from './PollTraceTimeline'
import './panels.css'

export interface ExecutionDetailPanelProps {
  nodeId: string
  execution: NodeExecutionDto | null
}

function JsonSection(props: { title: string; value: unknown; testId: string }) {
  if (props.value === null || props.value === undefined) {
    return null
  }
  return (
    <section className="panel-section" data-testid={props.testId}>
      <h3 className="panel-section__title">{props.title}</h3>
      <pre className="panel-code">{formatDisplayValue(props.value)}</pre>
    </section>
  )
}

function looksLikeHttpResolved(value: unknown): boolean {
  return (
    value !== null &&
    typeof value === 'object' &&
    'method' in value &&
    'path' in value &&
    typeof (value as { method: unknown }).method === 'string'
  )
}

function httpResponseBody(output: unknown): Record<string, unknown> | null {
  if (
    output === null ||
    typeof output !== 'object' ||
    Array.isArray(output) ||
    !('status' in output)
  ) {
    return null
  }
  return output
}

function AssertionRowView({ row }: { row: AssertionRow }) {
  return (
    <>
      <div className="assertion-list__head">
        <span>{row.path}</span>
        <span className={row.passed ? 'assertion-list__pass' : 'assertion-list__fail'}>
          {row.passed ? '通过' : '失败'}
        </span>
      </div>
      {row.operator ? <div className="assertion-list__op">op: {row.operator}</div> : null}
      {!row.passed ? (
        <div className="assertion-diff" data-testid="assertion-diff">
          <div className="assertion-diff__line assertion-diff__line--expected">
            <span className="assertion-diff__label">期望</span>
            <code>{formatDisplayValue(row.expected)}</code>
          </div>
          <div className="assertion-diff__line assertion-diff__line--actual">
            <span className="assertion-diff__label">实际</span>
            <code>{formatDisplayValue(row.actual)}</code>
          </div>
        </div>
      ) : null}
      {row.message ? <p className="assertion-list__message">{row.message}</p> : null}
    </>
  )
}

export function ExecutionDetailPanel({ nodeId, execution }: ExecutionDetailPanelProps) {
  if (!execution) {
    return (
      <div className="execution-detail execution-detail--empty" data-testid="execution-detail">
        <p className="execution-detail__placeholder">选择节点以查看执行详情</p>
      </div>
    )
  }

  const timingLines = formatTimingBreakdown(timingBreakdown(execution))
  const pollTrace = parsePollTrace(execution.poll_trace)
  const assertions = extractAssertionRows(execution)
  const hasHttp = looksLikeHttpResolved(execution.input_resolved)
  const response = hasHttp ? httpResponseBody(execution.output) : null

  return (
    <div className="execution-detail" data-testid="execution-detail">
      <div className="execution-detail__header">
        <h2 className="execution-detail__title">{nodeId}</h2>
        <span
          className={`execution-detail__status execution-detail__status--${execution.status.toLowerCase()}`}
        >
          {execution.status}
        </span>
      </div>

      {execution.failure ? (
        <p className="execution-detail__failure" data-testid="execution-failure">
          {execution.failure.category} · {execution.failure.error_code}
        </p>
      ) : null}

      {timingLines.length > 0 ? (
        <section className="panel-section" data-testid="execution-timing">
          <h3 className="panel-section__title">耗时</h3>
          <ul className="execution-detail__timing">
            {timingLines.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </section>
      ) : null}

      {hasHttp ? (
        <JsonSection title="实际请求" value={execution.input_resolved} testId="execution-request" />
      ) : null}

      {response ? (
        <JsonSection title="实际响应" value={response} testId="execution-response" />
      ) : null}

      {pollTrace.length > 0 ? <PollTraceTimeline nodeId={nodeId} entries={pollTrace} /> : null}

      {assertions.length > 0 ? (
        <section className="panel-section" data-testid="execution-assertions">
          <h3 className="panel-section__title">断言结果</h3>
          <ul className="assertion-list">
            {assertions.map((row) => (
              <li
                key={row.path}
                className={`assertion-list__item${row.passed ? '' : ' assertion-list__item--failed'}`}
                data-testid={`assertion-${row.path}`}
              >
                <AssertionRowView row={row} />
              </li>
            ))}
          </ul>
        </section>
      ) : null}
    </div>
  )
}
