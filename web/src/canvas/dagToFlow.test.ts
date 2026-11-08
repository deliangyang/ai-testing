import { ASYNC_POLL_DAG } from '../data/fixtures/asyncPoll'
import { PARALLEL_DAG } from '../data/fixtures/parallel'
import { SERIAL_DAG } from '../data/fixtures/serial'
import { buildDagEditGraph } from './dagToFlow'

describe('buildDagEditGraph', () => {
  it('is deterministic for the same DAG', () => {
    const a = JSON.stringify(buildDagEditGraph(SERIAL_DAG))
    const b = JSON.stringify(buildDagEditGraph(SERIAL_DAG))
    expect(a).toBe(b)
  })

  it.each([
    ['serial', SERIAL_DAG, 3, 2],
    ['parallel', PARALLEL_DAG, 4, 4],
    ['async-poll', ASYNC_POLL_DAG, 4, 3],
  ] as const)('lays out %s (%i nodes, %i edges)', (_name, dag, nodeCount, edgeCount) => {
    const { nodes, edges } = buildDagEditGraph(dag)
    expect(nodes).toHaveLength(nodeCount)
    expect(edges).toHaveLength(edgeCount)
    for (const node of nodes) {
      expect(node.position.x).toBeTypeOf('number')
      expect(node.position.y).toBeTypeOf('number')
      expect(node.data.summary.length).toBeGreaterThan(0)
    }
    for (const edge of edges) {
      expect(edge.markerEnd).toBeDefined()
    }
  })

  it('labels variable flow on create → pay', () => {
    const edge = buildDagEditGraph(SERIAL_DAG).edges.find(
      (e) => e.source === 'create' && e.target === 'pay',
    )
    expect(edge?.label).toBe('output.body.id')
  })
})
