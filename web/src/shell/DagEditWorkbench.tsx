import { useCallback, useEffect, useState } from 'react'

import { DagEditCanvas } from '../canvas'
import { FixtureDataSource } from '../data/fixture'
import type { GateResponse } from '../data/types'
import { DagValidationPanel, NodePropertyPanel } from '../panels'
import type { Dag } from '../types/generated'
import './shell.css'

const dataSource = new FixtureDataSource('serial')

export function DagEditWorkbench() {
  const [dag, setDag] = useState<Dag | null>(null)
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null)
  const [focusNodeId, setFocusNodeId] = useState<string | null>(null)
  const [gateReport, setGateReport] = useState<GateResponse | null>(null)
  const [repairing, setRepairing] = useState(false)
  const [repairHint, setRepairHint] = useState<string | null>(null)

  useEffect(() => {
    void dataSource.getActiveDag('serial').then((detail) => {
      setDag(detail.dag)
    })
  }, [])

  const saveDag = useCallback((next: Dag) => dataSource.saveDag(next), [])

  const repairWithAi = useCallback(async () => {
    if (!dag) {
      return
    }
    setRepairing(true)
    setRepairHint(null)
    try {
      const result = await dataSource.repairDagWithAi(dag)
      setDag(result.dag)
      setGateReport(result.gate)
      if (result.status === 'accepted') {
        setRepairHint(
          result.pending
            ? `已写入待审 v${String(result.pending.version)}，请激活后执行`
            : '闸门已通过',
        )
      } else {
        setRepairHint('仍需人工介入')
      }
    } catch (e) {
      setRepairHint(e instanceof Error ? e.message : 'AI 修正失败')
    } finally {
      setRepairing(false)
    }
  }, [dag])

  useEffect(() => {
    if (!dag) {
      return
    }
    let cancelled = false
    void dataSource.validateDag(dag).then((report) => {
      if (!cancelled) {
        setGateReport(report)
      }
    })
    return () => {
      cancelled = true
    }
  }, [dag])

  if (!dag) {
    return <div data-testid="dag-edit-workbench-loading">加载 DAG…</div>
  }

  return (
    <div className="dag-edit-workbench" data-testid="dag-edit-workbench">
      <main className="dag-edit-workbench__canvas">
        <DagEditCanvas
          dag={dag}
          onDagChange={setDag}
          onSelectedNodeIdChange={setSelectedNodeId}
          focusNodeId={focusNodeId}
        />
      </main>
      <aside className="dag-edit-workbench__panel" data-testid="dag-edit-property-panel">
        <DagValidationPanel
          report={gateReport}
          onFocusNode={(nodeId) => {
            setFocusNodeId(nodeId)
            setSelectedNodeId(nodeId)
          }}
          onAiRepair={() => {
            void repairWithAi()
          }}
          repairing={repairing}
          repairHint={repairHint}
        />
        <NodePropertyPanel
          dag={dag}
          selectedNodeId={selectedNodeId}
          onDagChange={setDag}
          onSave={saveDag}
        />
      </aside>
    </div>
  )
}
