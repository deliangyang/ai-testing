import { useCallback, useEffect, useState } from 'react'

import { DagEditCanvas } from '../canvas'
import { FixtureDataSource } from '../data/fixture'
import { NodePropertyPanel } from '../panels'
import type { Dag } from '../types/generated'
import './shell.css'

const dataSource = new FixtureDataSource('serial')

export function DagEditWorkbench() {
  const [dag, setDag] = useState<Dag | null>(null)
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null)

  useEffect(() => {
    void dataSource.getActiveDag('serial').then((detail) => {
      setDag(detail.dag)
    })
  }, [])

  const saveDag = useCallback((next: Dag) => dataSource.saveDag(next), [])

  if (!dag) {
    return (
      <div data-testid="dag-edit-workbench-loading">加载 DAG…</div>
    )
  }

  return (
    <div className="dag-edit-workbench" data-testid="dag-edit-workbench">
      <main className="dag-edit-workbench__canvas">
        <DagEditCanvas
          dag={dag}
          onDagChange={setDag}
          onSelectedNodeIdChange={setSelectedNodeId}
        />
      </main>
      <aside className="dag-edit-workbench__panel" data-testid="dag-edit-property-panel">
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
