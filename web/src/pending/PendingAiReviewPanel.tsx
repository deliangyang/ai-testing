import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react'

import { DagEditCanvas } from '../canvas'
import type { DataSource } from '../data/datasource'
import type { DagDetailDto, DagVersionDto, GateResponse } from '../data/types'
import { DagValidationPanel } from '../panels'
import type { Dag } from '../types/generated'
import { diffDags } from './dagDiff'
import './pending.css'

export interface PendingAiReviewPanelProps {
  dataSource: DataSource
  /** 固定展示某条待审（测试/E2E）；省略则加载列表首项。 */
  initialPending?: DagDetailDto
}

export function PendingAiReviewPanel({ dataSource, initialPending }: PendingAiReviewPanelProps) {
  const [pendingMeta, setPendingMeta] = useState<DagVersionDto | null>(
    initialPending
      ? {
          id: initialPending.id,
          version: initialPending.version,
          status: initialPending.status,
          created_by: initialPending.created_by,
          created_at: initialPending.created_at,
        }
      : null,
  )
  const [candidate, setCandidate] = useState<Dag | null>(initialPending?.dag ?? null)
  const [activeDag, setActiveDag] = useState<Dag | null>(null)
  const [activeVersion, setActiveVersion] = useState<number | null>(null)
  const [gateReport, setGateReport] = useState<GateResponse | null>(null)
  const [focusNodeId, setFocusNodeId] = useState<string | null>(null)
  const [activating, setActivating] = useState(false)
  const [activated, setActivated] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (initialPending) {
      return
    }
    let loadGeneration = 0
    loadGeneration += 1
    const generation = loadGeneration
    void dataSource.listPendingAiDags().then(async (rows) => {
      if (generation !== loadGeneration || rows.length === 0) {
        return
      }
      const first = rows[0]
      const detail = await dataSource.getDagVersion(first.id, first.version)
      if (generation !== loadGeneration) {
        return
      }
      setPendingMeta(first)
      setCandidate(detail.dag)
    })
    return () => {
      loadGeneration += 1
    }
  }, [dataSource, initialPending])

  useEffect(() => {
    if (!pendingMeta) {
      return
    }
    let cancelled = false
    void dataSource.getActiveDag(pendingMeta.id).then((detail) => {
      if (!cancelled) {
        setActiveDag(detail.dag)
        setActiveVersion(detail.version)
      }
    })
    return () => {
      cancelled = true
    }
  }, [dataSource, pendingMeta])

  useEffect(() => {
    if (!candidate) {
      return
    }
    let cancelled = false
    void dataSource.validateDag(candidate).then((report) => {
      if (!cancelled) {
        setGateReport(report)
      }
    })
    return () => {
      cancelled = true
    }
  }, [candidate, dataSource])

  const diff = useMemo(() => {
    if (!activeDag || !candidate) {
      return []
    }
    return diffDags(activeDag, candidate)
  }, [activeDag, candidate])

  const activate = useCallback(async () => {
    if (!pendingMeta || !gateReport?.passed) {
      return
    }
    setActivating(true)
    setError(null)
    try {
      await dataSource.activateDag(pendingMeta.id, pendingMeta.version)
      setActivated(true)
    } catch (e) {
      setError(e instanceof Error ? e.message : '激活失败')
    } finally {
      setActivating(false)
    }
  }, [dataSource, gateReport?.passed, pendingMeta])

  if (!candidate || !pendingMeta) {
    return <div data-testid="pending-ai-loading">加载 AI 候选…</div>
  }

  return (
    <div className="pending-ai-workbench" data-testid="pending-ai-workbench">
      <section className="pending-ai-workbench__canvas">
        <AiPreviewLayer>
          <DagEditCanvas dag={candidate} focusNodeId={focusNodeId} />
        </AiPreviewLayer>
      </section>
      <aside className="pending-ai-workbench__aside" data-testid="pending-ai-aside">
        <header>
          <h2>
            AI 候选 · {pendingMeta.id} v{pendingMeta.version}
          </h2>
          <p>与当前 active v{activeVersion ?? '—'} 对比</p>
        </header>
        <DagValidationPanel
          report={gateReport}
          onFocusNode={(nodeId) => {
            setFocusNodeId(nodeId)
          }}
        />
        <section data-testid="pending-ai-diff">
          <h3>版本差异</h3>
          {diff.length === 0 ? (
            <p>与 active 无节点差异</p>
          ) : (
            <ul className="pending-ai-diff__list">
              {diff.map((row) => (
                <li
                  key={`${row.kind}-${row.node_id}`}
                  className={`pending-ai-diff__row pending-ai-diff__row--${row.kind}`}
                  data-testid={`pending-diff-${row.kind}-${row.node_id}`}
                >
                  {row.kind === 'added' ? '+' : row.kind === 'removed' ? '−' : '~'} {row.node_id}:{' '}
                  {row.detail}
                </li>
              ))}
            </ul>
          )}
        </section>
        <button
          type="button"
          className="pending-ai-activate"
          data-testid="pending-ai-activate"
          disabled={activating || activated || !gateReport?.passed}
          onClick={() => void activate()}
        >
          {activated ? '已激活' : activating ? '激活中…' : '激活候选'}
        </button>
        {error ? (
          <p role="alert" data-testid="pending-ai-error">
            {error}
          </p>
        ) : null}
        {activated ? <p data-testid="pending-ai-activated">候选已成为 active 版本</p> : null}
      </aside>
    </div>
  )
}

function AiPreviewLayer({ children }: { children: ReactNode }) {
  return (
    <div className="dag-edit-canvas--ai-preview" data-testid="pending-ai-preview-layer">
      {children}
    </div>
  )
}
