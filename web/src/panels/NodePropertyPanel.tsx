import { useMemo, useState, type ReactNode } from 'react'

import type { DagVersionDto } from '../data/types'
import type { Condition, Dag, HttpMethod, Node, Operator } from '../types/generated'
import { upstreamVariableSuggestions } from './dagAncestors'
import { TemplateField } from './TemplateField'
import { findNode, replaceNode } from './updateDagNode'
import './panels.css'

const HTTP_METHODS: HttpMethod[] = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS']
const OPERATORS: Operator[] = [
  'eq',
  'ne',
  'gt',
  'gte',
  'lt',
  'lte',
  'contains',
  'exists',
  'not_exists',
]

function jsonScalarFieldText(value: unknown): string {
  if (value === null || value === undefined) {
    return ''
  }
  if (typeof value === 'string') {
    return value
  }
  if (typeof value === 'number' || typeof value === 'boolean') {
    return String(value)
  }
  return JSON.stringify(value)
}

export interface NodePropertyPanelProps {
  dag: Dag
  selectedNodeId: string | null
  onDagChange: (dag: Dag) => void
  onSave: (dag: Dag) => Promise<DagVersionDto>
}

function Field(props: { label: string; children: ReactNode }) {
  return (
    <label className="node-property__field">
      <span className="node-property__label">{props.label}</span>
      {props.children}
    </label>
  )
}

function HttpFields({
  dag,
  node,
  onChange,
}: {
  dag: Dag
  node: Extract<Node, { type: 'http' }>
  onChange: (node: Node) => void
}) {
  const suggestions = upstreamVariableSuggestions(dag, node.id)
  return (
    <>
      <Field label="方法">
        <select
          data-testid="prop-http-method"
          value={node.method}
          onChange={(e) => {
            onChange({ ...node, method: e.target.value as HttpMethod })
          }}
        >
          {HTTP_METHODS.map((m) => (
            <option key={m} value={m}>
              {m}
            </option>
          ))}
        </select>
      </Field>
      <TemplateField
        label="路径"
        testId="prop-http-path"
        value={node.path}
        suggestions={suggestions}
        onChange={(path) => {
          onChange({ ...node, path })
        }}
      />
      <Field label="名称">
        <input
          data-testid="prop-http-name"
          type="text"
          value={node.name ?? ''}
          onChange={(e) => {
            onChange({ ...node, name: e.target.value || undefined })
          }}
        />
      </Field>
    </>
  )
}

function PollFields({
  dag,
  node,
  onChange,
}: {
  dag: Dag
  node: Extract<Node, { type: 'poll' }>
  onChange: (node: Node) => void
}) {
  const condition = node.condition
  const suggestions = upstreamVariableSuggestions(dag, node.id)
  return (
    <>
      <Field label="轮询方法">
        <select
          data-testid="prop-poll-method"
          value={node.request.method}
          onChange={(e) => {
            onChange({
              ...node,
              request: { ...node.request, method: e.target.value as HttpMethod },
            })
          }}
        >
          {HTTP_METHODS.map((m) => (
            <option key={m} value={m}>
              {m}
            </option>
          ))}
        </select>
      </Field>
      <TemplateField
        label="轮询路径"
        testId="prop-poll-path"
        value={node.request.path}
        suggestions={suggestions}
        onChange={(path) => {
          onChange({
            ...node,
            request: { ...node.request, path },
          })
        }}
      />
      <Field label="条件 path">
        <input
          data-testid="prop-poll-condition-path"
          type="text"
          value={condition.path}
          onChange={(e) => {
            onChange({
              ...node,
              condition: { ...condition, path: e.target.value },
            })
          }}
        />
      </Field>
      <Field label="条件 op">
        <select
          data-testid="prop-poll-condition-op"
          value={condition.op}
          onChange={(e) => {
            onChange({
              ...node,
              condition: { ...condition, op: e.target.value as Operator },
            })
          }}
        >
          {OPERATORS.map((op) => (
            <option key={op} value={op}>
              {op}
            </option>
          ))}
        </select>
      </Field>
      <Field label="期望值">
        <input
          data-testid="prop-poll-expected"
          type="text"
          value={jsonScalarFieldText(condition.expected)}
          onChange={(e) => {
            onChange({
              ...node,
              condition: {
                ...condition,
                expected: e.target.value as unknown as Condition['expected'],
              },
            })
          }}
        />
      </Field>
      <Field label="超时 ms">
        <input
          data-testid="prop-poll-timeout"
          type="number"
          value={node.timeout_ms}
          onChange={(e) => {
            onChange({ ...node, timeout_ms: Number(e.target.value) })
          }}
        />
      </Field>
      <Field label="退避 ms（逗号分隔）">
        <input
          data-testid="prop-poll-backoff"
          type="text"
          value={node.backoff_ms.join(',')}
          onChange={(e) => {
            const backoff_ms = e.target.value
              .split(',')
              .map((s) => s.trim())
              .filter(Boolean)
              .map((s) => Number(s))
            onChange({ ...node, backoff_ms })
          }}
        />
      </Field>
    </>
  )
}

function AssertFields({
  node,
  onChange,
}: {
  node: Extract<Node, { type: 'assert' }>
  onChange: (node: Node) => void
}) {
  const assertion = node.assertions[0] ?? { path: '$.status', op: 'eq', expected: 200 }
  return (
    <>
      <Field label="目标节点">
        <input
          data-testid="prop-assert-target"
          type="text"
          value={node.target}
          onChange={(e) => {
            onChange({ ...node, target: e.target.value })
          }}
        />
      </Field>
      <Field label="断言 path">
        <input
          data-testid="prop-assert-path"
          type="text"
          value={assertion.path}
          onChange={(e) => {
            const next = { ...assertion, path: e.target.value }
            onChange({ ...node, assertions: [next, ...node.assertions.slice(1)] })
          }}
        />
      </Field>
      <Field label="断言 op">
        <select
          data-testid="prop-assert-op"
          value={assertion.op}
          onChange={(e) => {
            const next = { ...assertion, op: e.target.value as Operator }
            onChange({ ...node, assertions: [next, ...node.assertions.slice(1)] })
          }}
        >
          {OPERATORS.map((op) => (
            <option key={op} value={op}>
              {op}
            </option>
          ))}
        </select>
      </Field>
      <Field label="期望值">
        <input
          data-testid="prop-assert-expected"
          type="text"
          value={jsonScalarFieldText(assertion.expected)}
          onChange={(e) => {
            const raw = e.target.value
            const expected =
              raw === ''
                ? undefined
                : ((Number.isNaN(Number(raw))
                    ? raw
                    : Number(raw)) as unknown as Condition['expected'])
            const next: Condition = { ...assertion, expected }
            onChange({ ...node, assertions: [next, ...node.assertions.slice(1)] })
          }}
        />
      </Field>
    </>
  )
}

export function NodePropertyPanel({
  dag,
  selectedNodeId,
  onDagChange,
  onSave,
}: NodePropertyPanelProps) {
  const node = useMemo(
    () => (selectedNodeId ? findNode(dag, selectedNodeId) : undefined),
    [dag, selectedNodeId],
  )
  const [saving, setSaving] = useState(false)
  const [lastSave, setLastSave] = useState<DagVersionDto | null>(null)
  const [error, setError] = useState<string | null>(null)

  if (!node) {
    return (
      <div className="node-property node-property--empty" data-testid="node-property-panel">
        选中画布上的节点以编辑属性
      </div>
    )
  }

  const patch = (next: Node) => {
    onDagChange(replaceNode(dag, next))
  }

  return (
    <div className="node-property" data-testid="node-property-panel" data-node-type={node.type}>
      <header className="node-property__header">
        <h2 className="node-property__title">{node.id}</h2>
        <span className="node-property__type">{node.type}</span>
      </header>
      <div className="node-property__form">
        {node.type === 'http' ? <HttpFields dag={dag} node={node} onChange={patch} /> : null}
        {node.type === 'poll' ? <PollFields dag={dag} node={node} onChange={patch} /> : null}
        {node.type === 'assert' ? <AssertFields node={node} onChange={patch} /> : null}
        {node.type !== 'http' && node.type !== 'poll' && node.type !== 'assert' ? (
          <p className="node-property__unsupported">此节点类型暂不支持在 UI 中编辑</p>
        ) : null}
      </div>
      <footer className="node-property__footer">
        <button
          type="button"
          className="node-property__save"
          data-testid="node-property-save"
          disabled={saving}
          onClick={() => {
            setSaving(true)
            setError(null)
            void onSave(dag)
              .then((saved) => {
                setLastSave(saved)
              })
              .catch((err: unknown) => {
                setError(err instanceof Error ? err.message : '保存失败')
              })
              .finally(() => {
                setSaving(false)
              })
          }}
        >
          {saving ? '保存中…' : '保存为 pending 版本'}
        </button>
        {lastSave ? (
          <p
            className="node-property__save-result"
            data-testid="dag-pending-save"
            data-version={String(lastSave.version)}
            data-status={lastSave.status}
          >
            已保存 v{lastSave.version}（{lastSave.status}）
          </p>
        ) : null}
        {error ? <p className="node-property__error">{error}</p> : null}
      </footer>
    </div>
  )
}
