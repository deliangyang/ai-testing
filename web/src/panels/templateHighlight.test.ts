import { describe, expect, it } from 'vitest'

import { splitTemplateSegments } from './templateHighlight'

describe('splitTemplateSegments', () => {
  it('marks template spans', () => {
    expect(splitTemplateSegments('/orders/${create.output.body.id}/pay')).toEqual([
      { kind: 'text', text: '/orders/' },
      { kind: 'var', text: '${create.output.body.id}' },
      { kind: 'text', text: '/pay' },
    ])
  })
})
