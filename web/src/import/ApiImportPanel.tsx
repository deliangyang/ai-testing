import { useCallback, useMemo, useState } from 'react'

import type {
  ApiImportCommitRequest,
  ApiImportFormat,
  ApiImportMergeStrategy,
  ApiImportPreviewItem,
  ApiImportPreviewResponse,
  ApiSpecDto,
} from '../data/types'
import type { DataSource } from '../data/datasource'
import { LOW_CONFIDENCE_THRESHOLD } from './tokens'
import './import.css'

export type ApiImportPanelProps = {
  dataSource: DataSource
  initialPreview?: ApiImportPreviewResponse
}

type RowState = ApiImportPreviewItem & { accepted: boolean }

const FORMATS: { id: ApiImportFormat; label: string }[] = [
  { id: 'openapi', label: 'OpenAPI' },
  { id: 'postman', label: 'Postman' },
  { id: 'curl', label: 'cURL' },
  { id: 'har', label: 'HAR' },
  { id: 'md', label: 'Markdown' },
]

const STRATEGIES: { id: ApiImportMergeStrategy; label: string }[] = [
  { id: 'merge', label: '合并' },
  { id: 'overwrite', label: '覆盖' },
  { id: 'skip', label: '跳过已有' },
]

function mergeActionLabel(action: ApiImportPreviewItem['merge_action']): string {
  switch (action) {
    case 'add':
      return '新增'
    case 'update':
      return '更新'
    case 'unchanged':
      return '无变化'
    case 'skip':
      return '将跳过'
  }
}

export function ApiImportPanel({ dataSource, initialPreview }: ApiImportPanelProps) {
  const [format, setFormat] = useState<ApiImportFormat>('md')
  const [strategy, setStrategy] = useState<ApiImportMergeStrategy>('merge')
  const [content, setContent] = useState('')
  const [rows, setRows] = useState<RowState[]>(() =>
    (initialPreview?.items ?? []).map((item) => ({ ...item, accepted: true })),
  )
  const [summary, setSummary] = useState(initialPreview?.summary ?? null)
  const [commitMessage, setCommitMessage] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const loadPreview = useCallback(async () => {
    setBusy(true)
    setError(null)
    setCommitMessage(null)
    try {
      const preview = await dataSource.previewApiImport({
        format,
        content,
        strategy,
      })
      setSummary(preview.summary)
      setRows(preview.items.map((item) => ({ ...item, accepted: true })))
    } catch (e) {
      setError(e instanceof Error ? e.message : '预览失败')
    } finally {
      setBusy(false)
    }
  }, [content, dataSource, format, strategy])

  const commit = useCallback(async () => {
    setBusy(true)
    setError(null)
    try {
      const body: ApiImportCommitRequest = {
        strategy,
        items: rows.map((row) => ({
          spec: row.spec,
          accepted: row.accepted,
        })),
      }
      const result = await dataSource.commitApiImport(body)
      setCommitMessage(
        `已入库：新增 ${String(result.added)}，更新 ${String(result.updated)}，无变化 ${String(result.unchanged)}`,
      )
    } catch (e) {
      setError(e instanceof Error ? e.message : '入库失败')
    } finally {
      setBusy(false)
    }
  }, [dataSource, rows, strategy])

  const lowConfidenceIds = useMemo(
    () =>
      new Set(rows.filter((r) => r.confidence < LOW_CONFIDENCE_THRESHOLD).map((r) => r.spec.id)),
    [rows],
  )

  const patchSpec = (id: string, patch: Partial<ApiSpecDto>) => {
    setRows((prev) =>
      prev.map((row) => (row.spec.id === id ? { ...row, spec: { ...row.spec, ...patch } } : row)),
    )
  }

  return (
    <div className="api-import" data-testid="api-import-panel">
      <div className="api-import__toolbar">
        <div className="api-import__field">
          <label htmlFor="import-format">格式</label>
          <select
            id="import-format"
            data-testid="import-format"
            value={format}
            onChange={(e) => {
              setFormat(e.target.value as ApiImportFormat)
            }}
          >
            {FORMATS.map((f) => (
              <option key={f.id} value={f.id}>
                {f.label}
              </option>
            ))}
          </select>
        </div>
        <fieldset className="api-import__strategy" data-testid="import-strategy">
          <legend>合并策略</legend>
          {STRATEGIES.map((s) => (
            <label key={s.id}>
              <input
                type="radio"
                name="import-strategy"
                value={s.id}
                checked={strategy === s.id}
                onChange={() => {
                  setStrategy(s.id)
                }}
              />
              {s.label}
            </label>
          ))}
        </fieldset>
        <button
          type="button"
          data-testid="import-preview"
          disabled={busy}
          onClick={() => void loadPreview()}
        >
          生成预览
        </button>
        <button
          type="button"
          data-testid="import-commit"
          disabled={busy || rows.length === 0}
          onClick={() => void commit()}
        >
          确认入库
        </button>
      </div>

      <textarea
        data-testid="import-content"
        aria-label="导入原文"
        rows={6}
        value={content}
        onChange={(e) => {
          setContent(e.target.value)
        }}
        placeholder="粘贴 OpenAPI、Postman、cURL、HAR 或 Markdown 内容…"
      />

      {error ? (
        <p role="alert" data-testid="import-error">
          {error}
        </p>
      ) : null}
      {commitMessage ? <p data-testid="import-commit-result">{commitMessage}</p> : null}
      {summary ? (
        <p data-testid="import-summary">
          预览：新增 {summary.add} · 更新 {summary.update} · 无变化 {summary.unchanged} · 跳过{' '}
          {summary.skip}
        </p>
      ) : null}

      <div className="api-import__cards">
        {rows.map((row) => {
          const low = lowConfidenceIds.has(row.spec.id)
          return (
            <article
              key={row.spec.id}
              className={`api-import-card${low ? ' api-import-card--low-confidence' : ''}`}
              data-testid={`import-card-${row.spec.id}`}
              data-low-confidence={low ? 'true' : 'false'}
            >
              <header className="api-import-card__head">
                <strong>
                  {row.spec.method} {row.spec.path}
                </strong>
                <span className="api-import-card__meta">
                  置信度 {(row.confidence * 100).toFixed(0)}% · {mergeActionLabel(row.merge_action)}
                  {row.source_lines.length > 0 ? ` · 行 ${row.source_lines.join(',')}` : null}
                </span>
                <label>
                  <input
                    type="checkbox"
                    data-testid={`import-accept-${row.spec.id}`}
                    checked={row.accepted}
                    onChange={(e) => {
                      const accepted = e.target.checked
                      setRows((prev) =>
                        prev.map((r) => (r.spec.id === row.spec.id ? { ...r, accepted } : r)),
                      )
                    }}
                  />
                  采纳
                </label>
              </header>
              <div className="api-import-card__inputs">
                <input
                  aria-label="方法"
                  data-testid={`import-method-${row.spec.id}`}
                  value={row.spec.method}
                  onChange={(e) => {
                    patchSpec(row.spec.id, { method: e.target.value })
                  }}
                />
                <input
                  aria-label="路径"
                  data-testid={`import-path-${row.spec.id}`}
                  value={row.spec.path}
                  onChange={(e) => {
                    patchSpec(row.spec.id, { path: e.target.value })
                  }}
                />
                <input
                  aria-label="名称"
                  data-testid={`import-name-${row.spec.id}`}
                  value={row.spec.name}
                  onChange={(e) => {
                    patchSpec(row.spec.id, { name: e.target.value })
                  }}
                />
              </div>
            </article>
          )
        })}
      </div>
    </div>
  )
}
