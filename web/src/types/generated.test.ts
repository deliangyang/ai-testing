import type { Dag } from './generated'

describe('generated types', () => {
  it('accepts a minimal dag document', () => {
    const doc: Dag = {
      id: 'smoke',
      nodes: [
        {
          id: 'ping',
          type: 'http',
          method: 'GET',
          path: '/health',
        },
      ],
      edges: [],
    }
    expect(doc.id).toBe('smoke')
  })
})
