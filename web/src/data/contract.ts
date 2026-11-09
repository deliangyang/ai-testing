import { expect } from 'vitest'

import type { DataSource } from './datasource'
import { SERIAL_RUN } from './fixtures/serial'

/** Shared behavioural checks for every `DataSource` implementation. */
export async function exerciseDataSourceContract(make: () => DataSource): Promise<void> {
  const ds = make()

  const dags = await ds.listDags()
  expect(dags.some((d) => d.id === 'serial')).toBe(true)

  const pendingAi = await ds.listPendingAiDags()
  expect(Array.isArray(pendingAi)).toBe(true)

  const active = await ds.getActiveDag('serial')
  expect(active.dag.id).toBe('serial')
  expect(active.dag.nodes.length).toBeGreaterThan(0)

  const versioned = await ds.getDagVersion('serial', active.version)
  expect(versioned.dag.id).toBe('serial')

  const runs = await ds.listRuns({ dag_id: 'serial' })
  expect(runs.length).toBeGreaterThan(0)

  const run = await ds.getRun(SERIAL_RUN.id)
  expect(run.dag_id).toBe('serial')

  const executions = await ds.listExecutions(SERIAL_RUN.id)
  expect(executions.length).toBe(3)
  expect(executions.map((e) => e.node_id).sort()).toEqual(['check', 'create', 'pay'])

  const events: string[] = []
  await new Promise<void>((resolve, reject) => {
    try {
      const stop = ds.subscribeRunEvents(
        SERIAL_RUN.id,
        (event) => {
          events.push(event.type)
          if (event.type === 'run_finished') {
            stop()
            resolve()
          }
        },
        { since: 0 },
      )
    } catch (error) {
      reject(error instanceof Error ? error : new Error(String(error)))
    }
  })
  expect(events.at(-1)).toBe('run_finished')

  const gate = await ds.validateDag(active.dag)
  expect(gate.passed).toBe(true)

  const broken = structuredClone(active.dag)
  const pay = broken.nodes.find((n) => n.id === 'pay')
  if (pay && pay.type === 'http') {
    pay.path = '/broken/${missing.output.body}'
  }
  const failGate = await ds.validateDag(broken)
  expect(failGate.passed).toBe(false)
  const repair = await ds.repairDagWithAi(broken)
  expect(repair.gate.passed).toBe(true)

  const preview = await ds.previewApiImport({ format: 'openapi', content: '{}', strategy: 'merge' })
  expect(preview.items).toBeDefined()

  const commit = await ds.commitApiImport({
    strategy: 'merge',
    items: [],
  })
  expect(commit.added).toBeGreaterThanOrEqual(0)
}
