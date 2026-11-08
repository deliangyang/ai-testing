import type { Dag } from '../../types/generated'
import { SERIAL_DAG } from '../../data/fixtures/serial'
import { layoutDag } from './layout'

const serialDag: Dag = SERIAL_DAG

describe('layoutDag', () => {
  it('is deterministic for the same DAG', () => {
    const first = layoutDag(serialDag)
    const second = layoutDag(serialDag)
    expect(JSON.stringify(first)).toBe(JSON.stringify(second))
  })

  it('orders nodes by id in the result', () => {
    const layout = layoutDag(serialDag)
    const ids = layout.nodes.map((n) => n.id)
    expect(ids).toEqual(['check', 'create', 'pay'])
  })

  it('places upstream nodes above downstream nodes for serial flow', () => {
    const layout = layoutDag(serialDag)
    const byId = Object.fromEntries(layout.nodes.map((n) => [n.id, n]))
    expect(byId.create.y).toBeLessThan(byId.pay.y)
    expect(byId.pay.y).toBeLessThan(byId.check.y)
  })

  it('ignores edge list order', () => {
    const shuffled: Dag = {
      ...serialDag,
      edges: [
        { from: 'pay', to: 'check' },
        { from: 'create', to: 'pay' },
      ],
    }
    expect(JSON.stringify(layoutDag(shuffled))).toBe(JSON.stringify(layoutDag(serialDag)))
  })
})
